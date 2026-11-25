//! Controller behaviour against the simulated QC45 (fake.rs).

use crate::controller::{Controller, LinkState};
use crate::fake::{peer_mac, FakeHeadset, FakeState};
use crate::log::Log;
use crate::worker::{Command, Worker};

fn connected(state: FakeState) -> (Controller, FakeHeadset, Log) {
    let headset = FakeHeadset::new(state);
    let log = Log::memory();
    let mut controller = Controller::new(Box::new(headset.clone()), log.clone());
    controller.start();
    assert_eq!(controller.state().link, LinkState::Connected);
    headset.state().requests.clear();
    (controller, headset, log)
}

#[test]
fn start_reads_everything() {
    let (c, _, log) = connected(FakeState::default());
    let s = c.state();
    assert_eq!(s.device_name, "Bose QC45");
    assert_eq!(s.firmware, "4.0.4-4360+de6a887");
    assert_eq!(s.battery, Some(80));
    assert!(!s.charging);
    assert_eq!(s.channel, 8);
    let names: Vec<&str> = s.modes.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["Quiet", "Aware", "Walk"]);
    assert_eq!(s.free_mode_slot, Some(3));
    assert_eq!(s.mode_index, Some(0));
    assert_eq!(s.eq_values(), [0, 0, 0]);
    assert!(s.multipoint && s.supports_multipoint && s.supports_sidetone && s.supports_eq);
    assert_eq!(s.peers.len(), 4);
    assert!(s.peers[0].is_local, "this PC sorts first");
    assert_eq!(s.active_peer.as_deref(), Some(peer_mac(1).as_str()));
    assert!(s.peer(&peer_mac(1)).unwrap().active);

    let lines = log.lines();
    assert!(lines.iter().any(|l| l.starts_with("session: QC Buddy ") && l.contains("Bose QC45 fw 4.0.4")));
    assert!(lines.iter().any(|l| l == "modes: Quiet, Aware, Walk; free slot 3"));
    // 2.3 is unsupported on a QC45: logged once in plain English.
    let charging: Vec<_> = lines.iter().filter(|l| l.contains("charging state")).collect();
    assert_eq!(charging, ["Headphones don't support reading charging state [2.3 get, error 4 unsupported function]"]);
}

#[test]
fn unsupported_reads_are_logged_once() {
    let (mut c, _, log) = connected(FakeState::default());
    c.poll();
    c.poll();
    assert_eq!(log.lines().iter().filter(|l| l.contains("charging state")).count(), 1);
}

#[test]
fn start_does_not_connect_when_headset_is_off() {
    let headset = FakeHeadset::new(FakeState { powered_off: true, ..FakeState::default() });
    let mut c = Controller::new(Box::new(headset), Log::memory());
    c.start();
    assert_eq!(c.state().link, LinkState::Disconnected);
}

#[test]
fn set_mode_plays_prompt() {
    let (mut c, headset, log) = connected(FakeState::default());
    c.set_mode(2);
    assert_eq!(c.state().mode_index, Some(2));
    assert_eq!(headset.state().mode_index, 2);
    let start = headset.state().requests.iter().find(|p| p.is(31, 3) && p.op == crate::protocol::Op::Start).cloned();
    assert_eq!(start.unwrap().payload, [2, 1]);
    assert!(log.lines().contains(&"mode -> Walk".to_string()));
    assert_eq!(c.state().busy, None);
}

#[test]
fn set_mode_failure_restores_previous_mode() {
    let (mut c, _, _) = connected(FakeState::default());
    c.set_mode(3); // the free slot can't be selected
    assert_eq!(c.state().mode_index, Some(0));
    assert!(c.state().error.as_deref().unwrap().contains("invalid data"));
}

#[test]
fn update_mode_and_wind_block() {
    let (mut c, headset, _) = connected(FakeState::default());
    c.update_mode(2, 4, false);
    let walk = c.state().modes.iter().find(|m| m.index == 2).unwrap().clone();
    assert_eq!((walk.raw_cnc, walk.wind_block, walk.name.as_str()), (4, false, "Walk"));
    assert_eq!(walk.anc_label(), "ANC 6/10");
    c.update_mode(2, 4, true);
    assert_eq!(c.state().modes[2].anc_label(), "ANC Auto · Wind Block");
    assert_eq!(headset.state().records[2][46], 1);
    // Quiet isn't editable: nothing is sent.
    headset.state().requests.clear();
    c.update_mode(0, 5, false);
    assert!(headset.state().requests.is_empty());
}

#[test]
fn create_custom_mode_uses_live_level_under_wind_block() {
    let (mut c, headset, log) = connected(FakeState { mode_index: 2, live_cnc: 6, ..FakeState::default() });
    c.create_custom_mode();
    let s = c.state();
    assert_eq!(s.modes.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Quiet", "Aware", "Walk", "Custom"]);
    assert_eq!(s.modes[3].raw_cnc, 6);
    assert_eq!(s.mode_index, Some(3));
    assert_eq!(s.free_mode_slot, None);
    assert_eq!(headset.state().mode_index, 3);
    assert!(log.lines().contains(&"created Custom mode in slot 3 cnc=6".to_string()));
}

#[test]
fn mode_list_retries_incomplete_getall() {
    let (mut c, _, log) = connected(FakeState { incomplete_getalls: 2, ..FakeState::default() });
    // The connect already consumed the cut-short streams and retried.
    assert_eq!(c.state().modes.len(), 3);
    let lines = log.lines();
    assert!(lines.contains(&"mode list incomplete (attempt 1)".to_string()));
    assert!(lines.contains(&"mode list incomplete (attempt 2)".to_string()));
    c.load_modes();
    assert_eq!(c.state().modes.len(), 3);
}

#[test]
fn mode_list_keeps_last_good_list() {
    let (mut c, headset, _) = connected(FakeState::default());
    headset.state().incomplete_getalls = 3;
    c.load_modes();
    assert_eq!(c.state().modes.len(), 3, "kept, not replaced by the fallback");
}

#[test]
fn eq_band_and_preset() {
    let (mut c, headset, _) = connected(FakeState::default());
    c.set_eq(0, 15); // clamped to +10
    assert_eq!(c.state().eq_values(), [10, 0, 0]);
    headset.state().requests.clear();
    c.apply_eq(&[10, 4, -3]);
    assert_eq!(c.state().eq_values(), [10, 4, -3]);
    // Only the two changed bands are written.
    let writes = headset.state().requests.iter().filter(|p| p.is(1, 7) && p.op == crate::protocol::Op::SetGet).count();
    assert_eq!(writes, 2);
    assert_eq!(headset.state().eq[2].2, -3);
}

#[test]
fn sidetone_and_multipoint() {
    let (mut c, headset, _) = connected(FakeState::default());
    c.set_sidetone(2);
    assert_eq!((c.state().sidetone, headset.state().sidetone), (2, 2));
    c.set_multipoint(false);
    assert!(!c.state().multipoint && !headset.state().multipoint);
}

#[test]
fn connect_peer_with_free_slot() {
    let mut fake = FakeState::default();
    fake.peers[0].connected = false; // MacBook off: one slot free
    fake.source = None;
    let (mut c, headset, _) = connected(fake);
    c.connect_peer(&peer_mac(3), false);
    assert!(c.state().peer(&peer_mac(3)).unwrap().connected);
    assert!(!headset.state().request_log().contains(&"4.2 start".to_string()), "nothing dropped");
}

#[test]
fn auto_drop_never_drops_this_pc() {
    // This PC and the MacBook hold both slots; connecting the phone drops the MacBook.
    let (mut c, headset, log) = connected(FakeState::default());
    c.connect_peer(&peer_mac(3), false);
    let fake = headset.state();
    assert!(!fake.peer(&peer_mac(1)).unwrap().connected, "MacBook dropped");
    assert!(fake.peer(&peer_mac(2)).unwrap().connected, "PC kept");
    assert!(fake.peer(&peer_mac(3)).unwrap().connected, "phone connected");
    let log_lines = fake.request_log();
    let drop = log_lines.iter().position(|l| l == "4.2 start").unwrap();
    let connect = log_lines.iter().position(|l| l == "4.1 start").unwrap();
    assert!(drop < connect, "drop happens before connect");
    drop_guard(fake);
    assert!(log.lines().iter().any(|l| l.starts_with("connectPeer Sam\u{2019}s iPhone: connected=2/2 atCapacity=true")));
    assert_eq!(c.state().active_peer.as_deref(), Some(peer_mac(3).as_str()));
    assert_eq!(c.state().peer_action, None);
}

fn drop_guard<T>(_: T) {}

#[test]
fn ask_which_device_to_drop() {
    let (mut c, headset, _) = connected(FakeState::default());
    c.set_ask_before_switch(true);
    c.connect_peer(&peer_mac(3), false);
    let pending = c.state().pending_switch.clone().expect("asks");
    assert_eq!(pending.candidates.len(), 2);
    assert!(!headset.state().request_log().contains(&"4.1 start".to_string()), "nothing sent yet");

    c.confirm_pending_switch(&peer_mac(1));
    assert_eq!(c.state().pending_switch, None);
    assert!(!headset.state().peer(&peer_mac(1)).unwrap().connected);
    assert!(headset.state().peer(&peer_mac(3)).unwrap().connected);
}

#[test]
fn disconnect_peer() {
    let (mut c, headset, _) = connected(FakeState::default());
    c.disconnect_peer(&peer_mac(1));
    assert!(!headset.state().peer(&peer_mac(1)).unwrap().connected);
    assert!(!c.state().peer(&peer_mac(1)).unwrap().connected);
    assert_eq!(c.state().error, None);
}

#[test]
fn connect_errors_read_plainly() {
    let mut fake = FakeState::default();
    fake.peers[0].connected = false;
    fake.connect_error = Some(8);
    let (mut c, _, _) = connected(fake);
    c.connect_peer(&peer_mac(4), false);
    assert_eq!(
        c.state().error.as_deref(),
        Some("Sam's MacBook Air didn't answer. Make sure it's awake, nearby, and Bluetooth is on.")
    );
}

#[test]
fn pc_audio_follows_connect_self() {
    let mut fake = FakeState::default();
    fake.peers[1].connected = false; // the PC has only the control link
    let (mut c, headset, log) = connected(fake);
    assert!(!c.state().pc_audio);
    c.connect_pc_audio(false);
    assert!(c.state().pc_audio);
    assert!(headset.state().peer(&peer_mac(2)).unwrap().connected);
    assert!(log.lines().contains(&"PC audio connected".to_string()));
    assert_eq!(c.state().busy, None);
}

#[test]
fn pc_audio_that_never_appears_is_reported() {
    let mut fake = FakeState::default();
    fake.peers[1].connected = false;
    fake.pc_audio_follows_local = false; // e.g. Windows doesn't bring up the endpoint
    let (mut c, _, log) = connected(fake);
    c.connect_pc_audio(false);
    assert!(!c.state().pc_audio);
    assert!(c.state().error.as_deref().unwrap().starts_with("Windows still isn't offering"));
    assert!(log.lines().contains(&"CONNECT self did not produce a Windows audio device".to_string()));
}

#[test]
fn pairing_mode_and_power_off() {
    let (mut c, headset, _) = connected(FakeState::default());
    c.enter_pairing_mode();
    assert!(c.state().pairing_mode && headset.state().pairing);
    c.power_off();
    assert!(headset.state().powered_off);
    assert_eq!(c.state().link, LinkState::Disconnected);
    assert_eq!(c.state().battery, None);
    assert_eq!(c.state().device_name, "Bose QC45", "name kept for the header");
}

#[test]
fn lost_link_is_noticed() {
    let (mut c, headset, log) = connected(FakeState::default());
    headset.state().control_open = false;
    c.poll();
    assert_eq!(c.state().link, LinkState::Disconnected);
    assert!(c.state().error.as_deref().unwrap().starts_with("Bluetooth connection lost"));
    assert!(log.lines().contains(&"Bluetooth connection closed".to_string()));
}

#[test]
fn health_line() {
    let (mut c, _, log) = connected(FakeState::default());
    c.log_health();
    assert_eq!(
        log.lines().last().unwrap(),
        "health: battery 80% · mode Quiet (ANC 10/10) · EQ 0/0/0 · sidetone 0 · multipoint on · audio yes · devices DESKTOP-QCTEST, Sam's MacBook Pro"
    );
}

#[test]
fn state_serializes_for_the_ui() {
    let (c, _, _) = connected(FakeState::default());
    let json = serde_json::to_value(c.state()).unwrap();
    assert_eq!(json["link"]["kind"], "connected");
    assert_eq!(json["battery"], 80);
    assert_eq!(json["modes"][2]["windBlock"], true);
    assert_eq!(json["peers"][0]["isLocal"], true);
}

#[test]
fn worker_serializes_commands_and_reports_state() {
    use std::sync::{Arc, Mutex};
    let headset = FakeHeadset::new(FakeState::default());
    let seen: Arc<Mutex<Vec<Option<u8>>>> = Arc::default();
    let sink = seen.clone();
    let worker = Worker::spawn(Controller::new(Box::new(headset.clone()), Log::memory()), move |s| {
        sink.lock().unwrap().push(s.mode_index);
    });
    worker.send(Command::Start);
    worker.send(Command::SetMode(1));
    worker.send(Command::SetMode(2));
    drop(worker); // joins after the queued commands
    assert_eq!(headset.state().mode_index, 2);
    assert_eq!(seen.lock().unwrap().last().copied().flatten(), Some(2));
}
