//! The Rust ↔ UI contract. The flyout's tests render from these JSON files,
//! which are real controller states (simulated headset) serialized the same
//! way the app sends them. If a field changes here, this test fails until the
//! fixtures are regenerated, and the UI tests then show what broke:
//!
//!     $env:UPDATE_FIXTURES=1; cargo test ui_fixtures

use std::path::PathBuf;

use crate::controller::{Controller, State};
use crate::fake::{peer_mac, FakeHeadset, FakeState};
use crate::log::Log;

fn controller(fake: FakeState) -> Controller {
    let mut c = Controller::new(Box::new(FakeHeadset::new(fake)), Log::memory());
    c.start();
    c
}

fn fixtures() -> Vec<(&'static str, State)> {
    let mut out = Vec::new();

    // This PC + MacBook hold both audio slots; MacBook playing.
    out.push(("connected", controller(FakeState::default()).state().clone()));

    // One slot free: other devices offer Connect, not Switch.
    let mut fake = FakeState::default();
    fake.peers[0].connected = false;
    fake.source = None;
    out.push(("slot-free", controller(fake).state().clone()));

    // This PC has only the control link (e.g. a Remote Desktop session).
    let mut fake = FakeState::default();
    fake.peers[1].connected = false;
    out.push(("control-only", controller(fake).state().clone()));

    // "Ask which" prompt while both slots are full.
    let mut c = controller(FakeState::default());
    c.set_ask_before_switch(true);
    c.connect_peer(&peer_mac(3), false);
    out.push(("pending-switch", c.state().clone()));

    // Pairing mode on, multipoint off.
    let mut c = controller(FakeState::default());
    c.set_multipoint(false);
    c.enter_pairing_mode();
    out.push(("pairing", c.state().clone()));

    // A connect that failed, with the error banner.
    let mut fake = FakeState::default();
    fake.peers[0].connected = false;
    fake.connect_error = Some(8);
    let mut c = controller(fake);
    c.connect_peer(&peer_mac(4), false);
    out.push(("error", c.state().clone()));

    // Low battery, Walk (Wind Block) selected.
    let mut c = controller(FakeState { battery: 15, mode_index: 2, ..FakeState::default() });
    c.refresh(false);
    out.push(("low-battery", c.state().clone()));

    // Headset off: not connected, but paired.
    out.push(("disconnected", controller(FakeState { powered_off: true, ..FakeState::default() }).state().clone()));

    out
}

#[test]
fn ui_fixtures() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../app/src/__fixtures__");
    let update = std::env::var_os("UPDATE_FIXTURES").is_some();
    let mut stale = Vec::new();
    for (name, state) in fixtures() {
        let json = serde_json::to_string_pretty(&state).unwrap() + "\n";
        let path = dir.join(format!("{name}.json"));
        if update {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, &json).unwrap();
            continue;
        }
        let on_disk = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
        if on_disk != json {
            stale.push(name);
        }
    }
    assert!(
        stale.is_empty(),
        "UI fixtures out of date: {stale:?}. Regenerate with UPDATE_FIXTURES=1 cargo test ui_fixtures, then run the UI tests (npm test in windows/app)."
    );
}
