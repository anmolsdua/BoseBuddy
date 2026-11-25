//! A simulated QC45 that answers BMAP requests like the real one (fw 4.0.4),
//! for unit tests and the CLI's `--fake` mode. Its state is shared, so tests
//! can inspect what the controller did and change the headset underneath it.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::models::Headphones;
use crate::platform::Platform;
use crate::protocol::{format_mac, from_hex, mac_bytes, parse_all, BmapError, Op, Packet};
use crate::transport::Transport;

pub const HEADSET_ADDRESS: &str = "78:2B:64:00:00:45";

#[derive(Clone, Debug)]
pub struct FakePeer {
    pub mac: [u8; 6],
    pub name: String,
    pub connected: bool,
    pub local: bool,
}

#[derive(Clone, Debug)]
pub struct FakeState {
    pub name: String,
    pub firmware: String,
    pub battery: u8,
    /// 47-byte 31.6 records, one per slot.
    pub records: Vec<Vec<u8>>,
    pub mode_index: u8,
    /// Level Wind Block is applying right now (1.5).
    pub live_cnc: u8,
    /// (min, max, current, band id)
    pub eq: Vec<(i8, i8, i8, u8)>,
    pub multipoint: bool,
    pub sidetone: u8,
    pub peers: Vec<FakePeer>,
    pub source: Option<[u8; 6]>,
    pub pairing: bool,
    pub powered_off: bool,
    /// Whether this PC has the headset's control channel open.
    pub control_open: bool,
    /// Windows shows the headset as an audio output while the local peer is connected.
    pub pc_audio_follows_local: bool,
    /// Requests the headset rejects with "unsupported function" (2.3 on a QC45).
    pub unsupported: Vec<(u8, u8)>,
    /// GetAll calls still to cut short (no 31.1 RESULT), as after a ModeConfig write.
    pub incomplete_getalls: u32,
    /// Error code for the next CONNECT (4.1).
    pub connect_error: Option<u8>,
    /// Requests the headset never answers (the controller times out).
    pub silent: Vec<(u8, u8)>,
    /// Devices stay connected after DISCONNECT (4.2), as a busy phone might.
    pub ignore_disconnect: bool,
    /// Channels where the connection is refused (another app holds them).
    pub taken_channels: Vec<u8>,
    /// The channel the headset speaks BMAP on; others connect but stay silent.
    pub bmap_channel: u8,
    /// Requests rejected when written (SETGET / START answered with error 6).
    pub reject_writes: Vec<(u8, u8)>,
    /// Every request received, in order.
    pub requests: Vec<Packet>,
}

fn record(hex: &str) -> Vec<u8> {
    from_hex(hex)
}

fn mac(last: u8) -> [u8; 6] {
    [0x10, 0, 0, 0, 0, last]
}

impl Default for FakeState {
    /// The QC45 on the Windows test PC: Quiet, Aware, Walk and a free slot;
    /// a MacBook (playing) and this PC connected; a phone and a laptop off.
    fn default() -> FakeState {
        FakeState {
            name: "Bose QC45".into(),
            firmware: "4.0.4-4360+de6a887".into(),
            battery: 80,
            records: vec![
                record("00 00 01 00 00 01 51 75 69 65 74 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 00 00 00 00 00"),
                record("01 00 02 00 00 01 41 77 61 72 65 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 0a 00 00 00 00"),
                record("02 00 15 01 01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 00 00 00 00 01"),
                record("03 00 00 01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 00 00 00 00 00"),
            ],
            mode_index: 0,
            live_cnc: 3,
            eq: vec![(-10, 10, 0, 0), (-10, 10, 0, 1), (-10, 10, 0, 2)],
            multipoint: true,
            sidetone: 0,
            peers: vec![
                FakePeer { mac: mac(1), name: "Sam's MacBook Pro".into(), connected: true, local: false },
                FakePeer { mac: mac(2), name: "DESKTOP-QCTEST".into(), connected: true, local: true },
                FakePeer { mac: mac(3), name: "Sam\u{2019}s iPhone".into(), connected: false, local: false },
                FakePeer { mac: mac(4), name: "Sam's MacBook Air".into(), connected: false, local: false },
            ],
            source: Some(mac(1)),
            pairing: false,
            powered_off: false,
            control_open: false,
            pc_audio_follows_local: true,
            unsupported: vec![(2, 3)],
            incomplete_getalls: 0,
            connect_error: None,
            silent: Vec::new(),
            ignore_disconnect: false,
            taken_channels: Vec::new(),
            bmap_channel: 8,
            reject_writes: Vec::new(),
            requests: Vec::new(),
        }
    }
}

impl FakeState {
    pub fn peer(&self, mac: &str) -> Option<&FakePeer> {
        let bytes = mac_bytes(mac);
        self.peers.iter().find(|p| p.mac[..] == bytes[..])
    }

    pub fn pc_audio(&self) -> bool {
        !self.powered_off && self.pc_audio_follows_local && self.peers.iter().any(|p| p.local && p.connected)
    }

    /// Requests seen as "fblock.function op", e.g. "4.2 start".
    pub fn request_log(&self) -> Vec<String> {
        self.requests.iter().map(|p| format!("{}.{} {}", p.fblock, p.function, p.op)).collect()
    }

    fn respond(&mut self, p: &Packet) -> Vec<Packet> {
        let reply = |op: Op, payload: Vec<u8>| Packet::new(p.fblock, p.function, op, &payload);
        let status = |payload: Vec<u8>| reply(Op::Status, payload);
        let error = |code: u8| vec![reply(Op::Error, vec![code])];
        if self.unsupported.contains(&(p.fblock, p.function)) {
            return error(4);
        }
        if self.silent.contains(&(p.fblock, p.function)) {
            return vec![];
        }
        if matches!(p.op, Op::SetGet | Op::Start) && self.reject_writes.contains(&(p.fblock, p.function)) {
            return error(6);
        }
        match (p.fblock, p.function, p.op) {
            (0, 1, Op::Get) => vec![status(b"1.1.0".to_vec())],
            (0, 5, Op::Get) => vec![status(self.firmware.as_bytes().to_vec())],
            (1, 2, Op::Get) => {
                let mut payload = vec![0];
                payload.extend(self.name.as_bytes());
                vec![status(payload)]
            }
            (2, 2, Op::Get) => vec![status(vec![self.battery, 0xFF, 0xFF, 0])],
            (5, 1, Op::Get) => vec![status(match self.source {
                Some(m) => [&[0, 2, 1][..], &m[..]].concat(),
                None => vec![0, 2, 0],
            })],
            (4, 4, Op::Get) => {
                let mut payload = vec![self.peers.len() as u8];
                for peer in &self.peers {
                    payload.extend(peer.mac);
                }
                vec![status(payload)]
            }
            (4, 5, Op::Get) => match self.peers.iter().find(|x| x.mac[..] == p.payload[..]) {
                Some(peer) => {
                    let flags = peer.connected as u8 | (peer.local as u8) << 1;
                    let mut payload = peer.mac.to_vec();
                    payload.extend([flags, 1, 3]);
                    payload.extend(peer.name.as_bytes());
                    vec![status(payload)]
                }
                None => error(11),
            },
            (4, 1, Op::Start) => {
                if let Some(code) = self.connect_error.take() {
                    return error(code);
                }
                let target = p.payload.get(1..7).unwrap_or_default().to_vec();
                match self.peers.iter_mut().find(|x| x.mac[..] == target[..]) {
                    Some(peer) => {
                        peer.connected = true;
                        self.source = Some(peer.mac);
                        vec![reply(Op::Result, vec![])]
                    }
                    None => error(11),
                }
            }
            (4, 2, Op::Start) if self.ignore_disconnect => vec![reply(Op::Result, vec![])],
            (4, 2, Op::Start) => {
                if let Some(peer) = self.peers.iter_mut().find(|x| x.mac[..] == p.payload[..]) {
                    peer.connected = false;
                    if self.source == Some(peer.mac) {
                        self.source = self.peers.iter().find(|x| x.connected).map(|x| x.mac);
                    }
                }
                vec![reply(Op::Result, vec![])]
            }
            (4, 8, Op::Start) => {
                self.pairing = true;
                vec![reply(Op::Result, vec![])]
            }
            (7, 4, Op::Start) => {
                self.powered_off = true;
                vec![]
            }
            (31, 1, Op::Start) => {
                let mut out: Vec<Packet> =
                    self.records.iter().map(|r| Packet::new(31, 6, Op::Status, r)).collect();
                if self.incomplete_getalls > 0 {
                    self.incomplete_getalls -= 1;
                    out.truncate(2);
                } else {
                    out.push(Packet::new(31, 1, Op::Result, &[]));
                }
                out
            }
            (31, 3, Op::Get) => vec![status(vec![self.mode_index])],
            (31, 3, Op::Start) => {
                let index = p.payload.first().copied().unwrap_or(0xFF);
                match self.records.iter().find(|r| r[0] == index) {
                    Some(r) if r[5] != 0 || index <= 1 => {
                        self.mode_index = index;
                        vec![reply(Op::Result, vec![index])]
                    }
                    _ => error(6),
                }
            }
            (31, 6, Op::SetGet) if p.payload.len() == 39 => {
                let w = &p.payload;
                let Some(r) = self.records.iter_mut().find(|r| r[0] == w[0]) else {
                    return error(6);
                };
                if r[3] == 0 {
                    return error(5);
                }
                r[1] = w[1];
                r[2] = w[2];
                r[5] = 1;
                r[6..38].copy_from_slice(&w[3..35]);
                let wind = w[38] != 0;
                r[42] = if wind { 0 } else { w[35] };
                r[43] = w[36];
                r[44] = w[37];
                r[46] = wind as u8;
                let updated = r.clone();
                vec![status(updated)]
            }
            (1, 5, Op::Get) => vec![status(vec![0, self.live_cnc])],
            (1, 7, Op::Get) => vec![status(self.eq_bytes())],
            (1, 7, Op::SetGet) if p.payload.len() == 2 => {
                let (value, id) = (p.payload[0] as i8, p.payload[1]);
                match self.eq.iter_mut().find(|b| b.3 == id) {
                    Some(band) if value >= band.0 && value <= band.1 => {
                        band.2 = value;
                        vec![status(self.eq_bytes())]
                    }
                    _ => error(6),
                }
            }
            (1, 10, Op::Get) => vec![status(vec![if self.multipoint { 0x07 } else { 0x05 }])],
            (1, 10, Op::SetGet) => {
                self.multipoint = p.payload.first().copied().unwrap_or(0) != 0;
                vec![status(vec![if self.multipoint { 0x07 } else { 0x05 }])]
            }
            (1, 11, Op::Get) => vec![status(vec![1, self.sidetone])],
            (1, 11, Op::SetGet) if p.payload.len() == 2 => {
                self.sidetone = p.payload[1];
                vec![status(vec![1, self.sidetone])]
            }
            _ => error(4),
        }
    }

    fn eq_bytes(&self) -> Vec<u8> {
        self.eq.iter().flat_map(|b| [b.0 as u8, b.1 as u8, b.2 as u8, b.3]).collect()
    }
}

/// Shared handle on the simulated headset.
#[derive(Clone, Default)]
pub struct FakeHeadset {
    state: Arc<Mutex<FakeState>>,
}

impl FakeHeadset {
    pub fn new(state: FakeState) -> FakeHeadset {
        FakeHeadset { state: Arc::new(Mutex::new(state)) }
    }

    pub fn state(&self) -> MutexGuard<'_, FakeState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The controller's end of the control channel to the fake headset.
struct FakeLink {
    headset: FakeHeadset,
    channel: u8,
}

impl Transport for FakeLink {
    fn exchange(
        &mut self,
        request: &[u8],
        expect_reply: bool,
        _drain: bool,
        _timeout: Duration,
    ) -> Result<Vec<u8>, BmapError> {
        let mut state = self.headset.state();
        if state.powered_off || !state.control_open {
            return Err(BmapError::NotConnected);
        }
        if self.channel != state.bmap_channel {
            // Connected, but nothing on this channel speaks BMAP.
            return if expect_reply { Err(BmapError::Timeout) } else { Ok(Vec::new()) };
        }
        let mut out = Vec::new();
        for packet in parse_all(request) {
            state.requests.push(packet.clone());
            for reply in state.respond(&packet) {
                out.extend(reply.encode());
            }
        }
        if !expect_reply {
            return Ok(Vec::new());
        }
        if out.is_empty() {
            return Err(BmapError::Timeout);
        }
        Ok(out)
    }

    fn channel(&self) -> u8 {
        self.channel
    }

    fn pause(&mut self, _duration: Duration) {}
}

impl Platform for FakeHeadset {
    fn paired_headphones(&mut self) -> Vec<Headphones> {
        let state = self.state();
        vec![Headphones {
            address: HEADSET_ADDRESS.into(),
            name: state.name.clone(),
            connected: !state.powered_off,
        }]
    }

    fn open(&mut self, address: &str, channels: &[u8]) -> Result<Box<dyn Transport>, BmapError> {
        crate::transport::open_first_answering(channels, |channel| {
            let mut state = self.state();
            if address != HEADSET_ADDRESS || state.powered_off {
                return Err(BmapError::Transport("connect timed out".into()));
            }
            if state.taken_channels.contains(&channel) {
                return Err(BmapError::Transport("connection refused".into()));
            }
            state.control_open = true;
            drop(state);
            Ok(Box::new(FakeLink { headset: self.clone(), channel }) as Box<dyn Transport>)
        })
    }

    fn pc_audio_available(&mut self, _name_hint: &str) -> bool {
        self.state().pc_audio()
    }
}

/// MAC of a fake peer by position (1 = MacBook, 2 = this PC, 3 = phone, 4 = laptop).
pub fn peer_mac(n: u8) -> String {
    format_mac(&mac(n))
}
