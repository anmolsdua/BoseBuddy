//! Runs the controller on its own thread. Commands arrive over a channel and
//! run one at a time, so requests to the headset never overlap. Polls every
//! 15 s while the flyout is open; while it's closed, only the light
//! background tick (attach when Windows connects the headset, battery every
//! 5 min for the tray). The state goes to `on_state` after every command and
//! during long actions.
//!
//! The queue is bounded: `send` reports Busy rather than lining up clicks
//! behind a slow Bluetooth call. A panic in one command is logged and shown
//! as an error, and the worker carries on. Dropping the worker skips queued
//! commands and waits at most SHUTDOWN_WAIT for the one in progress.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::controller::{Controller, State};

pub const POLL_INTERVAL: Duration = Duration::from_secs(15);
/// While the flyout is closed: check for the headset once a minute
/// (no Bluetooth traffic) ...
pub const BACKGROUND_TICK: Duration = Duration::from_secs(60);
/// ... and read the battery every 5 minutes for the tray icon.
pub const BATTERY_EVERY_TICKS: u32 = 5;

/// How often the worker polls; tests shorten it.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub poll: Duration,
    pub tick: Duration,
    pub battery_every: u32,
}

impl Default for Timing {
    fn default() -> Timing {
        Timing { poll: POLL_INTERVAL, tick: BACKGROUND_TICK, battery_every: BATTERY_EVERY_TICKS }
    }
}

/// Sent by the UI as JSON, e.g. `{"type": "setMode", "args": 2}` or
/// `{"type": "updateMode", "args": {"index": 2, "rawCnc": 4, "windBlock": false}}`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "type", content = "args", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    Start,
    FlyoutOpened,
    FlyoutClosed,
    Connect(Option<String>),
    Disconnect,
    Refresh,
    SetMode(u8),
    UpdateMode { index: u8, raw_cnc: u8, wind_block: bool },
    CreateCustomMode,
    SetEq { band_id: u8, value: i32 },
    ApplyEq(Vec<i32>),
    SetSidetone(u8),
    SetMultipoint(bool),
    ConnectPeer(String),
    DisconnectPeer(String),
    ConfirmSwitch { drop_mac: String },
    CancelSwitch,
    ConnectPcAudio,
    EnterPairingMode,
    ClearPairingMode,
    SetAskBeforeSwitch(bool),
    ClearError,
    PowerOff,
    Shutdown,
}

/// Commands waiting at most; beyond this `send` says the worker is busy
/// instead of queueing clicks that would run long after they were made.
pub const QUEUE_LIMIT: usize = 8;
/// How long quitting waits for the action in progress (a Bluetooth call
/// can't be interrupted); after that the thread is left to finish alone.
pub const SHUTDOWN_WAIT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendError {
    /// QUEUE_LIMIT commands are already waiting.
    Busy,
    /// The worker thread has stopped.
    Stopped,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SendError::Busy => "QC Buddy is still working on earlier changes. Try again in a moment.",
            SendError::Stopped => "QC Buddy's connection to the headphones stopped. Restart QC Buddy.",
        })
    }
}

impl std::error::Error for SendError {}

pub struct Worker {
    tx: SyncSender<Command>,
    stop: Arc<AtomicBool>,
    // In a Mutex so the Worker is Sync (the app shares it across threads).
    done: Mutex<Receiver<()>>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn spawn(controller: Controller, on_state: impl Fn(&State) + Send + Sync + Clone + 'static) -> Worker {
        Worker::spawn_with(controller, Timing::default(), on_state)
    }

    pub fn spawn_with(
        mut controller: Controller,
        timing: Timing,
        on_state: impl Fn(&State) + Send + Sync + Clone + 'static,
    ) -> Worker {
        let (tx, rx) = mpsc::sync_channel::<Command>(QUEUE_LIMIT);
        let (done_tx, done) = mpsc::channel::<()>();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let progress = on_state.clone();
        controller.set_on_change(move |state| progress(state));
        let handle = std::thread::Builder::new()
            .name("qcbuddy-controller".into())
            .spawn(move || {
                let mut flyout_open = false;
                let mut next_poll = Instant::now() + timing.poll;
                let mut next_tick = Instant::now() + timing.tick;
                let mut ticks: u32 = 0;
                loop {
                    let deadline = if flyout_open { next_poll } else { next_tick };
                    let command = match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                        Ok(command) => Some(command),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => break,
                    };
                    // Quitting: don't start anything still queued.
                    if stopping.load(Ordering::SeqCst) || command == Some(Command::Shutdown) {
                        break;
                    }
                    let label = command.as_ref().map(|c| format!("{c:?}")).unwrap_or_else(|| "poll".into());
                    let outcome = catch_unwind(AssertUnwindSafe(|| match command {
                        None if flyout_open => {
                            controller.poll();
                            next_poll = Instant::now() + timing.poll;
                        }
                        None => {
                            ticks += 1;
                            controller.background_tick(ticks.is_multiple_of(timing.battery_every));
                            next_tick = Instant::now() + timing.tick;
                        }
                        Some(command) => {
                            match &command {
                                Command::FlyoutOpened => {
                                    flyout_open = true;
                                    next_poll = Instant::now() + timing.poll;
                                }
                                Command::FlyoutClosed => {
                                    flyout_open = false;
                                    next_tick = Instant::now() + timing.tick;
                                }
                                _ => {}
                            }
                            run(&mut controller, command);
                        }
                    }));
                    if let Err(panic) = outcome {
                        // A bug in one action mustn't take the app's headset connection down.
                        let message = panic
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                            .unwrap_or_default();
                        controller.recover_from_panic(&label, &message);
                    }
                    on_state(controller.state());
                }
                let _ = done_tx.send(());
            })
            .expect("spawn controller thread");
        Worker { tx, stop, done: Mutex::new(done), handle: Some(handle) }
    }

    /// Queue a command. Fails right away (never blocks the UI) when the
    /// queue is full or the worker has stopped.
    pub fn send(&self, command: Command) -> Result<(), SendError> {
        self.tx.try_send(command).map_err(|e| match e {
            TrySendError::Full(_) => SendError::Busy,
            TrySendError::Disconnected(_) => SendError::Stopped,
        })
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes an idle worker; with a full queue the worker sees `stop` instead.
        let _ = self.tx.try_send(Command::Shutdown);
        let finished = self.done.lock().map(|done| done.recv_timeout(SHUTDOWN_WAIT).is_ok()).unwrap_or(false);
        if finished {
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
        // Otherwise a Bluetooth call is still running: leave the thread to
        // finish on its own rather than hang the app's exit.
    }
}

/// Apply one command to the controller.
pub fn run(c: &mut Controller, command: Command) {
    match command {
        Command::Start => c.start(),
        Command::FlyoutOpened => c.flyout_opened(),
        Command::FlyoutClosed => c.flyout_closed(),
        Command::Connect(address) => c.connect(address.as_deref()),
        Command::Disconnect => c.disconnect(),
        Command::Refresh => c.refresh(true),
        Command::SetMode(index) => c.set_mode(index),
        Command::UpdateMode { index, raw_cnc, wind_block } => c.update_mode(index, raw_cnc, wind_block),
        Command::CreateCustomMode => c.create_custom_mode(),
        Command::SetEq { band_id, value } => c.set_eq(band_id, value),
        Command::ApplyEq(values) => c.apply_eq(&values),
        Command::SetSidetone(level) => c.set_sidetone(level),
        Command::SetMultipoint(on) => c.set_multipoint(on),
        Command::ConnectPeer(mac) => c.connect_peer(&mac, false),
        Command::DisconnectPeer(mac) => c.disconnect_peer(&mac),
        Command::ConfirmSwitch { drop_mac } => c.confirm_pending_switch(&drop_mac),
        Command::CancelSwitch => c.cancel_pending_switch(),
        Command::ConnectPcAudio => c.connect_pc_audio(false),
        Command::EnterPairingMode => c.enter_pairing_mode(),
        Command::ClearPairingMode => c.clear_pairing_mode(),
        Command::SetAskBeforeSwitch(ask) => c.set_ask_before_switch(ask),
        Command::ClearError => c.clear_error(),
        Command::PowerOff => c.power_off(),
        Command::Shutdown => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_from_ui_json() {
        let parse = |json: &str| serde_json::from_str::<Command>(json).unwrap();
        assert_eq!(parse(r#"{"type":"flyoutOpened"}"#), Command::FlyoutOpened);
        assert_eq!(parse(r#"{"type":"setMode","args":2}"#), Command::SetMode(2));
        assert_eq!(
            parse(r#"{"type":"updateMode","args":{"index":2,"rawCnc":4,"windBlock":false}}"#),
            Command::UpdateMode { index: 2, raw_cnc: 4, wind_block: false }
        );
        assert_eq!(parse(r#"{"type":"connect","args":null}"#), Command::Connect(None));
        assert_eq!(parse(r#"{"type":"applyEq","args":[4,0,-3]}"#), Command::ApplyEq(vec![4, 0, -3]));
    }
}

#[cfg(test)]
mod sync_tests {
    #[test]
    fn worker_can_be_shared_across_threads() {
        fn shareable<T: Send + Sync>() {}
        shareable::<super::Worker>();
    }
}
