//! The headset's own paired-device list (fblock 4) and audio source (5.1):
//! parsing and the multipoint switching rules from BoseController.swift.

use crate::models::{Peer, SourceKind};
use crate::protocol::{format_mac, string};

/// MACs from a 4.4 STATUS: one count byte, then 6 bytes per device.
pub fn parse_list(payload: &[u8]) -> Vec<String> {
    payload
        .get(1..)
        .unwrap_or_default()
        .chunks_exact(6)
        .filter(|chunk| chunk.iter().any(|b| *b != 0))
        .map(format_mac)
        .collect()
}

/// One device from a 4.5 STATUS: MAC (6), flags, 2 bytes, [1 more if Bose], name.
pub fn parse_info(mac: &str, payload: &[u8]) -> Peer {
    let mut peer = Peer::unknown(mac);
    if payload.len() < 7 {
        return peer;
    }
    let flags = payload[6];
    peer.connected = flags & 0x01 != 0;
    peer.is_local = flags & 0x02 != 0;
    peer.is_bose_product = flags & 0x04 != 0;
    let name_start = if peer.is_bose_product { 10 } else { 9 };
    if let Some(rest) = payload.get(name_start..) {
        let name = string(rest);
        if !name.is_empty() {
            peer.name = name;
        }
    }
    peer
}

/// Audio source from a 5.1 STATUS: `[?, ?, kind, mac(6)]`.
pub fn parse_source(payload: &[u8]) -> (SourceKind, Option<String>) {
    if payload.len() < 3 {
        return (SourceKind::None, None);
    }
    let kind = SourceKind::from_byte(payload[2]);
    let mac = (kind == SourceKind::Bluetooth && payload.len() >= 9).then(|| format_mac(&payload[3..9]));
    (kind, mac)
}

/// This PC first, then connected devices, then by name.
pub fn sort(peers: &mut [Peer]) {
    peers.sort_by(|a, b| {
        b.is_local
            .cmp(&a.is_local)
            .then(b.connected.cmp(&a.connected))
            .then_with(|| a.display_name().to_lowercase().cmp(&b.display_name().to_lowercase()))
    });
}

/// Whether a device holds one of the headset's two audio slots. The PC's
/// control link doesn't count: only its audio does.
pub fn holds_audio_slot(peer: &Peer, pc_audio: bool) -> bool {
    peer.connected || (peer.is_local && pc_audio)
}

/// Marks the device the headset is playing from.
pub fn mark_active(peers: &mut [Peer], active_mac: Option<&str>, pc_audio: bool) {
    for peer in peers {
        peer.active = Some(peer.mac.as_str()) == active_mac && holds_audio_slot(peer, pc_audio);
    }
}

/// What connecting `target` needs first.
#[derive(Debug, PartialEq, Eq)]
pub enum SlotPlan {
    /// A slot is free: just connect.
    Connect,
    /// Both slots are full: drop this device first (never this PC if avoidable).
    DropThenConnect(Peer),
    /// Both slots are full and the user asked to choose.
    Ask(Vec<Peer>),
}

pub fn plan_connect(peers: &[Peer], target: &Peer, pc_audio: bool, ask_which: bool) -> SlotPlan {
    let holding: Vec<&Peer> = peers.iter().filter(|p| holds_audio_slot(p, pc_audio)).collect();
    if holding.len() < 2 {
        return SlotPlan::Connect;
    }
    let candidates: Vec<Peer> = holding.into_iter().filter(|p| p.mac != target.mac).cloned().collect();
    if ask_which {
        return SlotPlan::Ask(candidates);
    }
    match candidates.iter().find(|p| !p.is_local).or(candidates.first()) {
        Some(victim) => SlotPlan::DropThenConnect(victim.clone()),
        None => SlotPlan::Connect,
    }
}

/// One-line summary for the log, same format as BoseBuddy's `peers:` line.
pub fn summary(peers: &[Peer], active_mac: Option<&str>) -> String {
    let list = peers
        .iter()
        .map(|p| {
            format!(
                "{}:{}{}",
                p.display_name(),
                if p.connected { "on" } else { "off" },
                if p.is_local { "/local" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("peers: {list} source={}", active_mac.unwrap_or("-"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::from_hex;

    // Replies captured from the QC45 (fw 4.0.4) on the Windows test PC, with
    // device names and MACs replaced (same flags, layout and encodings).
    pub const LIST: &str = "03 10 00 00 00 00 01 10 00 00 00 00 02 10 00 00 00 00 03 10 00 00 00 00 04";
    pub const MACBOOK: &str = "10 00 00 00 00 01 01 01 03 53 61 6d 27 73 20 4d 61 63 42 6f 6f 6b 20 50 72 6f";
    pub const THIS_PC: &str = "10 00 00 00 00 02 03 01 03 44 45 53 4b 54 4f 50 2d 51 43 54 45 53 54";
    pub const IPHONE: &str = "10 00 00 00 00 03 00 00 00 53 61 6d e2 80 99 73 20 69 50 68 6f 6e 65";
    pub const SOURCE: &str = "00 02 01 10 00 00 00 00 01";

    fn captured() -> Vec<Peer> {
        vec![
            parse_info("10:00:00:00:00:01", &from_hex(MACBOOK)),
            parse_info("10:00:00:00:00:02", &from_hex(THIS_PC)),
            parse_info("10:00:00:00:00:03", &from_hex(IPHONE)),
        ]
    }

    #[test]
    fn list_and_info() {
        let macs = parse_list(&from_hex(LIST));
        assert_eq!(macs, ["10:00:00:00:00:01", "10:00:00:00:00:02", "10:00:00:00:00:03", "10:00:00:00:00:04"]);
        assert!(parse_list(&from_hex("00 00 00 00 00 00 00")).is_empty());

        let peers = captured();
        assert_eq!(peers[0].name, "Sam's MacBook Pro");
        assert!(peers[0].connected && !peers[0].is_local);
        assert_eq!(peers[1].name, "DESKTOP-QCTEST");
        assert!(peers[1].connected && peers[1].is_local);
        assert_eq!(peers[2].name, "Sam\u{2019}s iPhone");
        assert!(!peers[2].connected);
        assert_eq!(parse_info("AA", &[1, 2]).name, "AA");
    }

    #[test]
    fn source_and_active_marker() {
        let (kind, mac) = parse_source(&from_hex(SOURCE));
        assert_eq!(kind, SourceKind::Bluetooth);
        assert_eq!(mac.as_deref(), Some("10:00:00:00:00:01"));
        assert_eq!(parse_source(&[0, 0]), (SourceKind::None, None));

        let mut peers = captured();
        mark_active(&mut peers, mac.as_deref(), false);
        assert!(peers[0].active && !peers[1].active);
        assert_eq!(peers[0].status_text(), "Connected · Audio");
    }

    #[test]
    fn sorting_and_summary() {
        let mut peers = captured();
        sort(&mut peers);
        let names: Vec<&str> = peers.iter().map(Peer::display_name).collect();
        assert_eq!(names, ["DESKTOP-QCTEST", "Sam's MacBook Pro", "Sam\u{2019}s iPhone"]);
        assert_eq!(
            summary(&peers, Some("10:00:00:00:00:01")),
            "peers: DESKTOP-QCTEST:on/local, Sam's MacBook Pro:on, Sam\u{2019}s iPhone:off source=10:00:00:00:00:01"
        );
    }

    #[test]
    fn auto_drop_rule() {
        let peers = captured();
        let iphone = peers[2].clone();
        // PC audio up + MacBook = both slots full: drop the MacBook, never the PC.
        assert_eq!(plan_connect(&peers, &iphone, true, false), SlotPlan::DropThenConnect(peers[0].clone()));
        // "Ask which" lists both slot holders.
        match plan_connect(&peers, &iphone, true, true) {
            SlotPlan::Ask(c) => assert_eq!(c.len(), 2),
            other => panic!("{other:?}"),
        }
        // The PC's control link alone (headset reports it not connected,
        // no Windows audio) doesn't take a slot.
        let mut control_only = peers.clone();
        control_only[1].connected = false;
        assert_eq!(plan_connect(&control_only, &iphone, false, false), SlotPlan::Connect);
        assert!(matches!(plan_connect(&control_only, &iphone, true, false), SlotPlan::DropThenConnect(_)));
    }
}
