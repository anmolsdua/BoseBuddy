//! Runs the controller on its own thread. Commands arrive over a channel and
//! run one at a time, so requests to the headset never overlap. Polls every
//! 15 s while the flyout is open; while it's closed, only the light
//! background tick (attach when Windows connects the headset, battery every
//! 5 min for the tray). The state goes to `on_state` after every command and
//! during long actions.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
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

pub struct Worker {
    tx: Sender<Command>,
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
        let (tx, rx) = mpsc::channel::<Command>();
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
                        Err(RecvTimeoutError::Disconnected) => return,
                    };
                    match command {
                        None if flyout_open => {
                            controller.poll();
                            next_poll = Instant::now() + timing.poll;
                        }
                        None => {
                            ticks += 1;
                            controller.background_tick(ticks.is_multiple_of(timing.battery_every));
                            next_tick = Instant::now() + timing.tick;
                        }
                        Some(Command::Shutdown) => return,
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
                    }
                    on_state(controller.state());
                }
            })
            .expect("spawn controller thread");
        Worker { tx, handle: Some(handle) }
    }

    pub fn send(&self, command: Command) {
        let _ = self.tx.send(command);
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
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
