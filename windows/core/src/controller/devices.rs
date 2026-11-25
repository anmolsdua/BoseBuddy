//! The headset's paired devices: connect, disconnect, switch, this PC's
//! audio, pairing mode, power off.

use super::*;

impl Controller {
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

    pub(super) fn perform_connect(&mut self, peer: &Peer) {
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
                // Only an error answering this CONNECT counts; success is
                // confirmed by polling the device's connected flag below.
                match reply_to(&packets, 4, 1) {
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
    pub(super) fn drop_peer(&mut self, peer: &Peer) {
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
}

pub(super) fn connect_message(error: &BmapError, peer: &Peer) -> String {
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
