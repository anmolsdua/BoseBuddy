//! Port of BoseController.swift: connection, state, reads and actions.
//!
//! The controller is synchronous and owns the control channel, so requests
//! can't overlap: `&mut self` is the request queue. `worker::Worker` runs it
//! on its own thread behind a command channel for the tray app.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::identity::{channel_candidates, friendly_name};
use crate::log::Log;
use crate::models::{EqBand, Headphones, Peer, PeerSwitch, SourceKind};
use crate::modes::BoseMode;
use crate::peers::{self, SlotPlan};
use crate::platform::Platform;
use crate::protocol::{mac_bytes, parse_all, product_name, string, BmapError, Op, Packet};
use crate::request_description;
use crate::transport::Transport;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum LinkState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

impl LinkState {
    pub fn is_connected(&self) -> bool {
        *self == LinkState::Connected
    }
}

/// Everything the UI renders. Sent whole after every change.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub link: LinkState,
    pub headphones: Vec<Headphones>,
    pub device_name: String,
    pub product_name: String,
    pub firmware: String,
    pub battery: Option<u8>,
    pub charging: bool,
    pub modes: Vec<BoseMode>,
    pub mode_index: Option<u8>,
    /// An editable mode slot with nothing in it, where a Custom mode can go.
    pub free_mode_slot: Option<u8>,
    pub eq_bands: Vec<EqBand>,
    pub multipoint: bool,
    pub sidetone: u8,
    pub peers: Vec<Peer>,
    pub active_peer: Option<String>,
    pub source: SourceKind,
    pub pairing_mode: bool,
    /// Windows has an active audio output for the headset.
    pub pc_audio: bool,
    pub supports_modes: bool,
    pub supports_eq: bool,
    pub supports_multipoint: bool,
    pub supports_sidetone: bool,
    pub supports_peers: bool,
    pub peers_loaded: bool,
    pub channel: u8,
    /// What's in progress: "mode", "mode-edit", "eq:0", "eq:preset",
    /// "multipoint", "sidetone", "pairing", "audio".
    pub busy: Option<String>,
    /// MAC of the device being connected or disconnected.
    pub peer_action: Option<String>,
    pub action_note: Option<String>,
    pub pending_switch: Option<PeerSwitch>,
    pub error: Option<String>,
    /// Ask which device to drop when both audio slots are full.
    pub ask_before_switch: bool,
}

impl Default for State {
    fn default() -> State {
        State {
            link: LinkState::Disconnected,
            headphones: Vec::new(),
            device_name: "Bose headphones".into(),
            product_name: String::new(),
            firmware: String::new(),
            battery: None,
            charging: false,
            modes: Vec::new(),
            mode_index: None,
            free_mode_slot: None,
            eq_bands: Vec::new(),
            multipoint: false,
            sidetone: 0,
            peers: Vec::new(),
            active_peer: None,
            source: SourceKind::None,
            pairing_mode: false,
            pc_audio: false,
            supports_modes: false,
            supports_eq: false,
            supports_multipoint: false,
            supports_sidetone: false,
            supports_peers: false,
            peers_loaded: false,
            channel: 0,
            busy: None,
            peer_action: None,
            action_note: None,
            pending_switch: None,
            error: None,
            ask_before_switch: false,
        }
    }
}

impl State {
    pub fn current_mode(&self) -> Option<&BoseMode> {
        self.modes.iter().find(|m| Some(m.index) == self.mode_index)
    }

    /// Band values ordered by band ID, for matching presets.
    pub fn eq_values(&self) -> Vec<i32> {
        self.eq_bands.iter().map(|b| b.value).collect()
    }

    pub fn peer(&self, mac: &str) -> Option<&Peer> {
        self.peers.iter().find(|p| p.mac == mac)
    }
}

const DEFAULT_TIMEOUT: Duration = Duration::from_millis(2800);

type StateCallback = Box<dyn FnMut(&State) + Send>;

pub struct Controller {
    state: State,
    platform: Box<dyn Platform>,
    link: Option<Box<dyn Transport>>,
    log: Log,
    /// Request failures already logged this connection, so a feature the
    /// headset doesn't support doesn't log on every poll.
    logged_failures: HashSet<String>,
    connected_address: Option<String>,
    light_polls: u32,
    on_change: Option<StateCallback>,
}

impl Controller {
    pub fn new(platform: Box<dyn Platform>, log: Log) -> Controller {
        Controller {
            state: State::default(),
            platform,
            link: None,
            log,
            logged_failures: HashSet::new(),
            connected_address: None,
            light_polls: 0,
            on_change: None,
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn log(&self) -> &Log {
        &self.log
    }

    /// Called with the new state while long actions are in progress
    /// ("Connecting…"), in addition to the worker's update after each command.
    pub fn set_on_change(&mut self, callback: impl FnMut(&State) + Send + 'static) {
        self.on_change = Some(Box::new(callback));
    }

    fn changed(&mut self) {
        if let Some(callback) = self.on_change.as_mut() {
            callback(&self.state);
        }
    }

    pub fn set_ask_before_switch(&mut self, ask: bool) {
        self.state.ask_before_switch = ask;
        if !ask {
            self.state.pending_switch = None;
        }
    }

    pub fn clear_error(&mut self) {
        self.state.error = None;
    }

    fn set_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        if self.state.error.as_deref() != Some(message.as_str()) {
            self.log.write(format!("error shown: {message}"));
        }
        self.state.error = Some(message);
    }

    // MARK: - Lifecycle

    /// App launch: attach to a headset that is already connected to this PC.
    /// Never starts a new Bluetooth connection, so opening QC Buddy can't
    /// displace another device.
    pub fn start(&mut self) {
        self.state.headphones = self.platform.paired_headphones();
        self.refresh_pc_audio();
        if let Some(connected) = self.state.headphones.iter().find(|h| h.connected).cloned() {
            self.connect(Some(&connected.address));
        } else if self.state.pc_audio && !self.state.headphones.is_empty() {
            self.connect(None);
        }
    }

    /// The flyout opened: re-read what can change from elsewhere.
    pub fn flyout_opened(&mut self) {
        self.refresh_pc_audio();
        self.refresh_paired_devices();
        if self.state.link.is_connected() {
            self.read_battery();
            self.read_source();
            self.load_peers();
            // Modes can be added, edited or deleted from the Bose app.
            self.read_mode_index();
            self.load_modes();
            // So can EQ, sidetone and multipoint (or BoseBuddy on another computer).
            self.load_eq();
            self.load_sidetone();
            self.load_multipoint();
            self.log_health();
        } else if self.state.link != LinkState::Connecting
            && (self.state.headphones.iter().any(|h| h.connected)
                || (self.state.pc_audio && !self.state.headphones.is_empty()))
        {
            self.connect(None);
        }
    }

    pub fn flyout_closed(&mut self) {
        self.state.pending_switch = None;
    }

    /// 15 s poll while the flyout is open.
    pub fn poll(&mut self) {
        self.refresh(false);
    }

    /// While the flyout is closed (every minute): attach to the headset once
    /// Windows has connected it, and with `read_battery` (every few minutes)
    /// keep the tray battery fresh. Never starts a Bluetooth connection, and
    /// retries a failed attach only on battery ticks.
    pub fn background_tick(&mut self, read_battery: bool) {
        match self.state.link {
            LinkState::Connected => {
                if read_battery {
                    self.read_battery();
                }
            }
            LinkState::Connecting => {}
            LinkState::Failed(_) if !read_battery => {}
            LinkState::Disconnected | LinkState::Failed(_) => {
                self.state.headphones = self.platform.paired_headphones();
                if let Some(connected) = self.state.headphones.iter().find(|h| h.connected).cloned() {
                    self.state.link = LinkState::Disconnected;
                    self.connect(Some(&connected.address));
                }
            }
        }
    }

    pub fn refresh_paired_devices(&mut self) {
        let found = self.platform.paired_headphones();
        if !self.state.link.is_connected() && !matches!(self.state.link, LinkState::Failed(_)) {
            if let Some(first) = found.first() {
                self.state.device_name = first.name.clone();
            }
        }
        self.state.headphones = found;
    }

    // MARK: - Connection

    pub fn connect(&mut self, address: Option<&str>) {
        if self.state.link.is_connected() || self.state.link == LinkState::Connecting {
            return;
        }
        self.state.link = LinkState::Connecting;
        self.state.error = None;
        self.state.pairing_mode = false;
        self.changed();

        if self.state.headphones.is_empty() {
            self.state.headphones = self.platform.paired_headphones();
        }
        let target = address.map(str::to_string).or_else(|| {
            self.state
                .headphones
                .iter()
                .find(|h| h.connected)
                .or(if self.state.pc_audio { self.state.headphones.first() } else { None })
                .map(|h| h.address.clone())
        });
        let Some(target) = target else {
            let message = "Connect the headphones from the side button or the Bose app on your phone, then try again.";
            self.state.link = LinkState::Failed(message.into());
            self.set_error(message);
            return;
        };

        let paired_name = self
            .state
            .headphones
            .iter()
            .find(|h| h.address == target)
            .map(|h| h.name.clone())
            .unwrap_or_default();
        if !paired_name.is_empty() {
            self.state.device_name = friendly_name(&paired_name);
        }
        self.refresh_pc_audio();

        match self.platform.open(&target, &channel_candidates(&paired_name)) {
            Ok(link) => self.attach(link, &target),
            Err(error) => {
                let message = error.to_string();
                self.log.write(format!("connect failed: {message}"));
                self.state.link = LinkState::Failed(message.clone());
                self.set_error(message);
            }
        }
        self.changed();
    }

    fn attach(&mut self, link: Box<dyn Transport>, address: &str) {
        let channel = link.channel();
        self.link = Some(link);
        self.state.channel = channel;
        self.connected_address = Some(address.to_string());
        self.state.link = LinkState::Connected;
        self.log.write(format!("Bluetooth connection established on channel {channel} to {address}"));
        self.changed();

        if let Ok(packet) = self.get(1, 2, &[]) {
            let name = product_name(&packet.payload);
            if !name.is_empty() {
                self.state.product_name = friendly_name(&name);
                self.state.device_name = self.state.product_name.clone();
            }
        }
        self.state.firmware = self.get(0, 5, &[]).map(|p| string(&p.payload).trim().to_string()).unwrap_or_default();
        self.logged_failures.clear();
        let name = if self.state.product_name.is_empty() { &self.state.device_name } else { &self.state.product_name };
        self.log.write(format!(
            "session: QC Buddy {APP_VERSION} · {} · {name} fw {} · channel {channel}",
            std::env::consts::OS,
            if self.state.firmware.is_empty() { "?" } else { &self.state.firmware },
        ));
        self.refresh(true);
    }

    pub fn disconnect(&mut self) {
        self.connected_address = None;
        self.link = None;
        let keep = State {
            headphones: std::mem::take(&mut self.state.headphones),
            device_name: std::mem::take(&mut self.state.device_name),
            product_name: std::mem::take(&mut self.state.product_name),
            firmware: std::mem::take(&mut self.state.firmware),
            pc_audio: self.state.pc_audio,
            ask_before_switch: self.state.ask_before_switch,
            ..State::default()
        };
        self.state = keep;
        self.changed();
    }

    /// The headset closed the control channel (or it failed).
    fn handle_control_closed(&mut self) {
        if !self.state.link.is_connected() {
            return;
        }
        self.log.write("Bluetooth connection closed");
        self.link = None;
        self.state.link = LinkState::Disconnected;
        self.state.channel = 0;
        self.set_error("Bluetooth connection lost. PC audio may be unaffected; click Connect to reconnect.");
        self.changed();
    }

    // MARK: - Refresh

    pub fn refresh(&mut self, deep: bool) {
        if !self.state.link.is_connected() {
            return;
        }
        self.read_battery();
        self.read_source();
        self.read_mode_index();
        if !deep {
            self.light_polls += 1;
            if self.light_polls.is_multiple_of(4) {
                self.load_peers();
            }
            return;
        }
        self.load_multipoint();
        self.load_sidetone();
        self.load_peers();
        self.load_modes();
        self.load_eq();
    }

    pub fn refresh_pc_audio(&mut self) {
        let hint = self.audio_name_hint();
        let available = self.platform.pc_audio_available(&hint);
        if available != self.state.pc_audio {
            self.state.pc_audio = available;
            self.log.write(format!("Windows audio available: {available}"));
            peers::mark_active(&mut self.state.peers, self.state.active_peer.as_deref(), available);
            self.changed();
        }
    }

    fn audio_name_hint(&self) -> String {
        self.state
            .headphones
            .iter()
            .find(|h| Some(&h.address) == self.connected_address.as_ref())
            .map(|h| h.name.clone())
            .unwrap_or_else(|| if self.state.device_name.is_empty() { "bose".into() } else { self.state.device_name.clone() })
    }

    // MARK: - Reads

    pub fn read_battery(&mut self) {
        if let Ok(packet) = self.get(2, 2, &[]) {
            if let Some(&level) = packet.payload.first() {
                self.state.battery = Some(level);
            }
            if let Ok(packet) = self.get(2, 3, &[]) {
                self.state.charging = packet.payload.first().is_some_and(|b| *b != 0);
            }
        }
    }

    fn read_mode_index(&mut self) {
        if let Ok(packet) = self.get(31, 3, &[]) {
            if let Some(&index) = packet.payload.first() {
                self.state.mode_index = (index != 0xFF).then_some(index);
            }
        }
    }

    fn read_source(&mut self) {
        let Ok(packet) = self.get(5, 1, &[]) else { return };
        let (kind, mac) = peers::parse_source(&packet.payload);
        self.state.source = kind;
        self.state.active_peer = mac;
        peers::mark_active(&mut self.state.peers, self.state.active_peer.as_deref(), self.state.pc_audio);
    }

    pub fn load_peers(&mut self) {
        if !self.state.link.is_connected() {
            return;
        }
        match self.get(4, 4, &[]) {
            Ok(list) => {
                let mut result: Vec<Peer> = peers::parse_list(&list.payload)
                    .into_iter()
                    .map(|mac| self.read_peer_info(&mac).unwrap_or_else(|_| Peer::unknown(&mac)))
                    .collect();
                peers::sort(&mut result);
                peers::mark_active(&mut result, self.state.active_peer.as_deref(), self.state.pc_audio);
                self.log.write(peers::summary(&result, self.state.active_peer.as_deref()));
                self.state.peers = result;
                self.state.supports_peers = true;
            }
            Err(_) => self.state.supports_peers = false,
        }
        self.state.peers_loaded = true;
    }

    fn read_peer_info(&mut self, mac: &str) -> Result<Peer, BmapError> {
        let packet = self.get(4, 5, &mac_bytes(mac))?;
        Ok(peers::parse_info(mac, &packet.payload))
    }

    /// GetAll streams one 31.6 record per slot and ends with a 31.1 RESULT.
    /// Right after a ModeConfig write the stream can time out or arrive cut
    /// short, so only a complete list is accepted, with retries.
    pub fn load_modes(&mut self) {
        if !self.state.link.is_connected() {
            return;
        }
        for attempt in 1..=3 {
            if let Ok(packets) = self.exchange(31, 1, Op::Start, &[], true, true, DEFAULT_TIMEOUT) {
                if packets.iter().any(|p| p.is(31, 1) && p.op == Op::Result) {
                    let records: Vec<&[u8]> = packets
                        .iter()
                        .filter(|p| p.is(31, 6) && p.op == Op::Status)
                        .map(|p| p.payload.as_slice())
                        .collect();
                    let mut found: Vec<BoseMode> = Vec::new();
                    for mode in records.iter().filter_map(|r| BoseMode::from_record(r)) {
                        found.retain(|m| m.index != mode.index);
                        found.push(mode);
                    }
                    found.sort_by_key(|m| m.index);
                    self.state.modes = if found.is_empty() { BoseMode::fallback() } else { found };
                    self.state.free_mode_slot =
                        records.iter().filter(|r| BoseMode::is_free_slot(r)).map(|r| r[0]).min();
                    self.state.supports_modes = true;
                    let names: Vec<&str> = self.state.modes.iter().map(|m| m.name.as_str()).collect();
                    self.log.write(format!(
                        "modes: {}{}",
                        names.join(", "),
                        self.state.free_mode_slot.map(|s| format!("; free slot {s}")).unwrap_or_default()
                    ));
                    return;
                }
            }
            self.log.write(format!("mode list incomplete (attempt {attempt})"));
            self.pause(Duration::from_millis(700));
        }
        // Keep what is on screen; only fall back when nothing was ever read.
        if self.state.modes.is_empty() {
            self.state.modes = BoseMode::fallback();
        }
    }

    pub fn load_eq(&mut self) {
        match self.get(1, 7, &[]) {
            Ok(packet) => {
                let names = ["Bass", "Mid", "Treble"];
                let mut bands: Vec<EqBand> = packet
                    .payload
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| (c[0] as i8 as i32, c[1] as i8 as i32, c[2] as i8 as i32, c[3]))
                    .filter(|(min, max, _, _)| max > min)
                    .map(|(minimum, maximum, value, id)| EqBand {
                        band_id: id,
                        name: names.get(id as usize).map(|s| s.to_string()).unwrap_or_else(|| format!("Band {id}")),
                        minimum,
                        maximum,
                        value,
                    })
                    .collect();
                if !bands.is_empty() {
                    bands.sort_by_key(|b| b.band_id);
                    self.state.eq_bands = bands;
                    self.state.supports_eq = true;
                }
            }
            Err(_) => {
                if self.state.eq_bands.is_empty() {
                    self.state.supports_eq = false;
                }
            }
        }
    }

    pub fn load_multipoint(&mut self) {
        match self.get(1, 10, &[]) {
            Ok(packet) => {
                self.state.multipoint = packet.payload.first().is_some_and(|b| b & 0x02 != 0);
                self.state.supports_multipoint = true;
            }
            Err(_) => self.state.supports_multipoint = false,
        }
    }

    pub fn load_sidetone(&mut self) {
        match self.get(1, 11, &[]) {
            Ok(packet) => {
                self.state.sidetone = packet.payload.get(1).copied().unwrap_or(0);
                self.state.supports_sidetone = true;
            }
            Err(_) => self.state.supports_sidetone = false,
        }
    }

    // MARK: - Noise modes

    pub fn set_mode(&mut self, index: u8) {
        if !self.state.link.is_connected() {
            return;
        }
        self.begin("mode");
        let previous = self.state.mode_index;
        self.state.mode_index = Some(index);
        // Second byte asks the headset to play the mode prompt, like the
        // physical Action button.
        let result = self
            .exchange(31, 3, Op::Start, &[index, 1], true, false, Duration::from_secs(3))
            .and_then(|packets| ensure_ok(&packets, 31, 3));
        match result {
            Ok(()) => {
                let name = self.mode_name(index);
                self.log.write(format!("mode -> {name}"));
                self.pause(Duration::from_millis(250));
                self.read_mode_index();
            }
            Err(error) => {
                self.state.mode_index = previous;
                self.set_error(error.to_string());
            }
        }
        self.end();
    }

    /// Rewrite an editable mode's noise cancelling. The headset keeps the
    /// change, so it survives reconnects and the Action button cycle.
    pub fn update_mode(&mut self, index: u8, raw_cnc: u8, wind_block: bool) {
        let Some(mode) = self.state.modes.iter().find(|m| m.index == index).cloned() else { return };
        if !self.state.link.is_connected() || !mode.editable {
            return;
        }
        self.begin("mode-edit");
        if let Some(m) = self.state.modes.iter_mut().find(|m| m.index == index) {
            m.raw_cnc = if wind_block { 0 } else { raw_cnc.min(10) };
            m.wind_block = wind_block;
        }
        let payload = mode.config_payload(raw_cnc, wind_block);
        match self
            .exchange(31, 6, Op::SetGet, &payload, true, false, DEFAULT_TIMEOUT)
            .and_then(|packets| ensure_ok(&packets, 31, 6))
        {
            Ok(()) => self.log.write(format!("mode {index} set cnc={raw_cnc} wind={wind_block}")),
            Err(error) => {
                self.log.write(format!("mode {index} write failed: {error}"));
                self.set_error(error.to_string());
            }
        }
        self.load_modes();
        self.end();
    }

    /// Create a mode named "Custom" in the free slot, starting from the
    /// current noise cancelling level, and switch to it.
    pub fn create_custom_mode(&mut self) {
        if !self.state.link.is_connected() || self.state.busy.is_some() {
            return;
        }
        let Some(slot) = self.state.free_mode_slot else { return };
        self.begin("mode-edit");
        let current = self.state.current_mode().cloned();
        let mut raw_cnc = current.as_ref().map_or(0, |m| m.raw_cnc);
        if current.as_ref().is_some_and(|m| m.wind_block) {
            // Wind Block modes store 0; use the level it is applying now.
            if let Ok(live) = self.get(1, 5, &[]) {
                if let Some(&level) = live.payload.get(1) {
                    raw_cnc = level;
                }
            }
        }
        let mode = BoseMode::custom(slot, raw_cnc);
        let result = self
            .exchange(31, 6, Op::SetGet, &mode.config_payload(raw_cnc, false), true, false, DEFAULT_TIMEOUT)
            .and_then(|packets| ensure_ok(&packets, 31, 6));
        if let Err(error) = result {
            self.log.write(format!("create Custom mode failed: {error}"));
            self.set_error(error.to_string());
            self.end();
            return;
        }
        self.log.write(format!("created Custom mode in slot {slot} cnc={raw_cnc}"));
        // Show it right away instead of waiting for the next GetAll.
        self.state.free_mode_slot = None;
        self.state.modes.retain(|m| m.index != slot);
        self.state.modes.push(mode);
        self.state.modes.sort_by_key(|m| m.index);
        self.end();
        self.set_mode(slot);
        self.load_modes();
    }

    fn mode_name(&self, index: u8) -> String {
        self.state.modes.iter().find(|m| m.index == index).map(|m| m.name.clone()).unwrap_or_else(|| index.to_string())
    }

    // MARK: - EQ, sidetone, multipoint

    pub fn set_eq(&mut self, band_id: u8, value: i32) {
        let Some(band) = self.state.eq_bands.iter().find(|b| b.band_id == band_id).cloned() else { return };
        if !self.state.link.is_connected() {
            return;
        }
        let clamped = band.clamp(value);
        self.begin(&format!("eq:{band_id}"));
        if let Some(b) = self.state.eq_bands.iter_mut().find(|b| b.band_id == band_id) {
            b.value = clamped;
        }
        match self.write_eq(band_id, clamped) {
            Ok(()) => self.log.write(format!("EQ {} -> {clamped}", band.name)),
            Err(error) => self.set_error(error.to_string()),
        }
        self.load_eq();
        self.end();
    }

    /// Apply a whole preset. Only bands that differ are written.
    pub fn apply_eq(&mut self, values: &[i32]) {
        if !self.state.link.is_connected() || self.state.eq_bands.is_empty() {
            return;
        }
        self.begin("eq:preset");
        let bands = self.state.eq_bands.clone();
        let mut result = Ok(());
        for (i, (band, value)) in bands.iter().zip(values).enumerate() {
            let clamped = band.clamp(*value);
            if clamped == band.value {
                continue;
            }
            self.state.eq_bands[i].value = clamped;
            result = self.write_eq(band.band_id, clamped);
            if result.is_err() {
                break;
            }
        }
        match result {
            Ok(()) => self.log.write(format!(
                "EQ preset -> {}",
                values.iter().map(i32::to_string).collect::<Vec<_>>().join("/")
            )),
            Err(error) => self.set_error(error.to_string()),
        }
        self.load_eq();
        self.end();
    }

    fn write_eq(&mut self, band_id: u8, value: i32) -> Result<(), BmapError> {
        let byte = value.clamp(i8::MIN as i32, i8::MAX as i32) as i8 as u8;
        let packets = self.exchange(1, 7, Op::SetGet, &[byte, band_id], true, false, DEFAULT_TIMEOUT)?;
        ensure_ok(&packets, 1, 7)
    }

    pub fn set_sidetone(&mut self, level: u8) {
        if !self.state.link.is_connected() {
            return;
        }
        self.begin("sidetone");
        self.state.sidetone = level;
        match self
            .exchange(1, 11, Op::SetGet, &[1, level], true, false, DEFAULT_TIMEOUT)
            .and_then(|packets| ensure_ok(&packets, 1, 11))
        {
            Ok(()) => {
                self.log.write(format!("sidetone -> {level}"));
                self.load_sidetone();
            }
            Err(error) => self.set_error(error.to_string()),
        }
        self.end();
    }

    pub fn set_multipoint(&mut self, enabled: bool) {
        if !self.state.link.is_connected() {
            return;
        }
        self.begin("multipoint");
        let previous = self.state.multipoint;
        self.state.multipoint = enabled;
        match self
            .exchange(1, 10, Op::SetGet, &[enabled as u8], true, false, DEFAULT_TIMEOUT)
            .and_then(|packets| ensure_ok(&packets, 1, 10))
        {
            Ok(()) => {
                self.log.write(format!("multipoint -> {}", if enabled { "on" } else { "off" }));
                self.pause(Duration::from_millis(200));
                self.load_multipoint();
                self.load_peers();
            }
            Err(error) => {
                self.state.multipoint = previous;
                self.set_error(error.to_string());
            }
        }
        self.end();
    }

    // MARK: - Devices

    /// Connect one of the headset's paired devices. With both audio slots
    /// full, drop a device that isn't this PC first, or ask which to drop.
    pub fn connect_peer(&mut self, mac: &str, force: bool) {
        let Some(peer) = self.state.peer(mac).cloned() else { return };
        if !self.state.link.is_connected() || self.state.peer_action.is_some() {
            return;
        }
        if peer.connected && !peer.is_local {
            return;
        }
        if peer.is_local && self.state.pc_audio {
            return;
        }
        let pc_audio = self.state.pc_audio;
        let holding = self.state.peers.iter().filter(|p| peers::holds_audio_slot(p, pc_audio)).count();
        let plan = peers::plan_connect(&self.state.peers, &peer, pc_audio, self.state.ask_before_switch && !force);
        let candidates: Vec<&str> = match &plan {
            SlotPlan::Ask(c) => c.iter().map(Peer::display_name).collect(),
            SlotPlan::DropThenConnect(v) => vec![v.display_name()],
            SlotPlan::Connect => vec![],
        };
        self.log.write(format!(
            "connectPeer {}: connected={holding}/2 atCapacity={} candidates={candidates:?}",
            peer.display_name(),
            holding >= 2
        ));
        match plan {
            SlotPlan::Ask(candidates) => {
                self.state.pending_switch = Some(PeerSwitch { target: peer, candidates });
                return;
            }
            SlotPlan::DropThenConnect(victim) => {
                self.state.peer_action = Some(peer.mac.clone());
                self.state.action_note = Some(format!("Switching from {}…", victim.display_name()));
                self.changed();
                self.drop_peer(&victim);
                self.load_peers();
                self.state.peer_action = None;
                self.state.action_note = None;
            }
            SlotPlan::Connect => {}
        }
        self.perform_connect(&peer);
    }

    fn perform_connect(&mut self, peer: &Peer) {
        if self.state.peer_action.is_some() {
            return;
        }
        self.state.peer_action = Some(peer.mac.clone());
        self.state.error = None;
        if peer.is_local {
            self.connect_pc_audio(false);
        } else {
            self.state.action_note = Some(format!("Connecting {}…", peer.display_name()));
            self.changed();
            let mut payload = vec![0u8];
            payload.extend(mac_bytes(&peer.mac));
            let result = self.exchange(4, 1, Op::Start, &payload, true, false, Duration::from_secs(4)).and_then(|packets| {
                match packets.iter().find(|p| p.is(4, 1)).or(packets.first()) {
                    Some(reply) if reply.op == Op::Error => Err(BmapError::Device {
                        code: reply.payload.first().copied().unwrap_or(0),
                        fblock: 4,
                        function: 1,
                    }),
                    _ => Ok(()),
                }
            });
            match result {
                Ok(()) => {
                    for _ in 0..14 {
                        self.pause(Duration::from_millis(500));
                        if self.read_peer_info(&peer.mac).is_ok_and(|p| p.connected) {
                            break;
                        }
                    }
                    self.read_source();
                    self.load_peers();
                }
                Err(error) => {
                    let message = connect_message(&error, peer);
                    self.set_error(message);
                }
            }
        }
        self.state.peer_action = None;
        self.state.action_note = None;
    }

    pub fn disconnect_peer(&mut self, mac: &str) {
        let Some(peer) = self.state.peer(mac).cloned() else { return };
        if !self.state.link.is_connected() || self.state.peer_action.is_some() {
            return;
        }
        if peer.is_local {
            if self.state.pc_audio {
                self.disconnect_pc_audio();
            }
            return;
        }
        if !peer.connected {
            return;
        }
        self.log.write(format!("disconnectPeer {}", peer.display_name()));
        self.state.peer_action = Some(peer.mac.clone());
        self.state.error = None;
        self.state.action_note = Some(format!("Disconnecting {}…", peer.display_name()));
        self.changed();
        self.drop_peer(&peer);
        self.read_source();
        self.load_peers();
        if self.state.peer(&peer.mac).is_some_and(|p| p.connected) {
            self.set_error(format!("Couldn't disconnect {}. Try again.", peer.display_name()));
        }
        self.state.peer_action = None;
        self.state.action_note = None;
    }

    /// "Ask which" answer: drop `drop_mac`, then connect the pending target.
    pub fn confirm_pending_switch(&mut self, drop_mac: &str) {
        let Some(pending) = self.state.pending_switch.take() else { return };
        let Some(victim) = pending.candidates.iter().find(|p| p.mac == drop_mac).cloned() else { return };
        self.state.peer_action = Some(victim.mac.clone());
        self.state.action_note = Some(format!("Disconnecting {}…", victim.display_name()));
        self.changed();
        if victim.is_local {
            self.state.peer_action = None;
            self.disconnect_pc_audio();
        } else {
            self.drop_peer(&victim);
        }
        self.load_peers();
        self.state.peer_action = None;
        self.state.action_note = None;
        self.connect_peer(&pending.target.mac, true);
    }

    pub fn cancel_pending_switch(&mut self) {
        self.state.pending_switch = None;
    }

    /// Sends DISCONNECT and waits for the connected flag to clear. Best effort.
    fn drop_peer(&mut self, peer: &Peer) {
        let _ = self.exchange(4, 2, Op::Start, &mac_bytes(&peer.mac), false, false, Duration::from_secs(2));
        for _ in 0..12 {
            self.pause(Duration::from_millis(400));
            if self.read_peer_info(&peer.mac).is_ok_and(|p| !p.connected) {
                break;
            }
        }
    }

    /// Ask the headset to bring this PC online as an audio device: the same
    /// CONNECT used for other devices, targeted at the PC's own entry.
    pub fn connect_pc_audio(&mut self, automatic: bool) {
        if !self.state.link.is_connected() {
            if !automatic {
                self.set_error("Connect the headphones first.");
            }
            return;
        }
        self.refresh_pc_audio();
        if self.state.pc_audio {
            return;
        }
        if !automatic {
            self.begin("audio");
        }
        if self.state.peers.is_empty() {
            self.load_peers();
        }
        let Some(local) = self.state.peers.iter().find(|p| p.is_local).cloned() else {
            if !automatic {
                self.set_error("Couldn't find this PC in the headphones' paired-device list.");
                self.end();
            }
            return;
        };
        let holding = self.state.peers.iter().filter(|p| peers::holds_audio_slot(p, false)).count();
        self.log.write(format!("connectPCAudio {}; connected={holding}/2", local.mac));
        self.state.action_note = Some("Connecting this PC…".into());
        self.changed();
        let mut payload = vec![0u8];
        payload.extend(mac_bytes(&local.mac));
        let reply = self.exchange(4, 1, Op::Start, &payload, true, false, Duration::from_secs(4));
        let ops: Vec<&str> = reply.as_ref().map(|ps| ps.iter().map(|p| p.op.name()).collect()).unwrap_or_default();
        self.log.write(format!("CONNECT self response: {ops:?}"));
        for _ in 0..12 {
            self.pause(Duration::from_millis(400));
            self.refresh_pc_audio();
            if self.state.pc_audio {
                break;
            }
        }
        self.state.action_note = None;
        if self.state.pc_audio {
            self.log.write("PC audio connected");
            self.read_source();
            self.load_peers();
        } else {
            self.log.write("CONNECT self did not produce a Windows audio device");
            if !automatic {
                self.set_error("Windows still isn't offering the headphones as an output. Connect them in Settings → Bluetooth & devices, then try again.");
            }
        }
        if !automatic {
            self.end();
        }
    }

    pub fn disconnect_pc_audio(&mut self) {
        if !self.state.link.is_connected() {
            return;
        }
        self.begin("audio");
        self.state.error = None;
        self.load_peers();
        let Some(local) = self.state.peers.iter().find(|p| p.is_local).cloned() else {
            self.end();
            return;
        };
        self.state.action_note = Some("Disconnecting this PC…".into());
        self.changed();
        let _ = self.exchange(4, 2, Op::Start, &mac_bytes(&local.mac), false, false, Duration::from_secs(2));
        for _ in 0..12 {
            self.pause(Duration::from_millis(400));
            self.refresh_pc_audio();
            if !self.state.pc_audio {
                break;
            }
        }
        self.state.action_note = None;
        self.load_peers();
        if self.state.pc_audio {
            self.set_error("Couldn't disconnect this PC's audio. Try the Bluetooth button on the headphones.");
        }
        self.end();
    }

    pub fn enter_pairing_mode(&mut self) {
        if !self.state.link.is_connected() {
            return;
        }
        self.begin("pairing");
        match self
            .exchange(4, 8, Op::Start, &[1], true, false, Duration::from_secs(3))
            .and_then(|packets| ensure_ok(&packets, 4, 8))
        {
            Ok(()) => self.state.pairing_mode = true,
            Err(error) => self.set_error(error.to_string()),
        }
        self.end();
    }

    pub fn clear_pairing_mode(&mut self) {
        self.state.pairing_mode = false;
    }

    pub fn power_off(&mut self) {
        if !self.state.link.is_connected() {
            return;
        }
        self.log.write("power off requested");
        let _ = self.exchange(7, 4, Op::Start, &[0], false, false, Duration::from_millis(500));
        self.disconnect();
    }

    /// One-line snapshot for the log, written when the flyout opens.
    pub fn log_health(&mut self) {
        let s = &self.state;
        let mode = s.current_mode().map(|m| format!("{} ({})", m.name, m.anc_label())).unwrap_or_else(|| "?".into());
        let devices: Vec<&str> = s.peers.iter().filter(|p| p.connected).map(Peer::display_name).collect();
        let line = format!(
            "health: battery {}{} · mode {mode} · EQ {} · sidetone {} · multipoint {} · audio {} · devices {}",
            s.battery.map(|b| format!("{b}%")).unwrap_or_else(|| "?".into()),
            if s.charging { " charging" } else { "" },
            s.eq_values().iter().map(i32::to_string).collect::<Vec<_>>().join("/"),
            s.sidetone,
            if s.multipoint { "on" } else { "off" },
            if s.pc_audio { "yes" } else { "no" },
            if devices.is_empty() { "-".to_string() } else { devices.join(", ") },
        );
        self.log.write(line);
    }

    // MARK: - Packet helpers

    fn begin(&mut self, action: &str) {
        self.state.busy = Some(action.to_string());
        self.changed();
    }

    fn end(&mut self) {
        self.state.busy = None;
    }

    fn pause(&mut self, duration: Duration) {
        if let Some(link) = self.link.as_mut() {
            link.pause(duration);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn exchange(
        &mut self,
        fblock: u8,
        function: u8,
        op: Op,
        payload: &[u8],
        expect_reply: bool,
        drain: bool,
        timeout: Duration,
    ) -> Result<Vec<Packet>, BmapError> {
        let Some(link) = self.link.as_mut() else { return Err(BmapError::NotConnected) };
        let request = Packet::new(fblock, function, op, payload).encode();
        let started = Instant::now();
        match link.exchange(&request, expect_reply, drain, timeout) {
            Ok(bytes) => {
                let packets = parse_all(&bytes);
                if let Some(reply) = packets.iter().find(|p| p.is(fblock, function) && p.op == Op::Error) {
                    let code = reply.payload.first().copied().unwrap_or(0);
                    self.log_failure(request_description::device_error(fblock, function, op, code));
                }
                Ok(packets)
            }
            Err(error) => {
                let seconds = started.elapsed().as_secs_f64();
                self.log_failure(request_description::failure(fblock, function, op, &error, seconds));
                if error == BmapError::NotConnected {
                    self.handle_control_closed();
                }
                Err(error)
            }
        }
    }

    fn get(&mut self, fblock: u8, function: u8, payload: &[u8]) -> Result<Packet, BmapError> {
        let packets = self.exchange(fblock, function, Op::Get, payload, true, false, DEFAULT_TIMEOUT)?;
        let reply = match packets.iter().position(|p| p.is(fblock, function)) {
            Some(i) => packets.into_iter().nth(i),
            None => packets.into_iter().next(),
        }
        .ok_or(BmapError::NoResponse)?;
        if reply.op == Op::Error {
            return Err(BmapError::Device { code: reply.payload.first().copied().unwrap_or(0), fblock, function });
        }
        Ok(reply)
    }

    fn log_failure(&mut self, line: String) {
        // Durations vary, so leave them out of the "already logged" key.
        let key = match (line.find(" after "), line.find("s [")) {
            (Some(a), Some(b)) if b > a => format!("{}{}", &line[..a], &line[b + 1..]),
            _ => line.clone(),
        };
        if self.logged_failures.insert(key) {
            self.log.write(line);
        }
    }
}

fn ensure_ok(packets: &[Packet], fblock: u8, function: u8) -> Result<(), BmapError> {
    match packets.iter().find(|p| p.is(fblock, function)).or(packets.first()) {
        Some(reply) if reply.op == Op::Error => {
            Err(BmapError::Device { code: reply.payload.first().copied().unwrap_or(0), fblock, function })
        }
        _ => Ok(()),
    }
}

fn connect_message(error: &BmapError, peer: &Peer) -> String {
    let name = peer.display_name();
    match error {
        BmapError::Device { code: 7, .. } => format!("{name} isn't available. Make sure it's nearby and Bluetooth is on."),
        BmapError::Device { code: 8, .. } => {
            format!("{name} didn't answer. Make sure it's awake, nearby, and Bluetooth is on.")
        }
        BmapError::Device { code: 11, .. } => format!("{name} is no longer paired with the headphones."),
        BmapError::Device { code: 12, .. } => "The headphones are busy. Try again in a moment.".into(),
        BmapError::Device { code, .. } => {
            format!("Couldn't connect {name} ({}).", crate::protocol::error_name(*code))
        }
        other => other.to_string(),
    }
}
