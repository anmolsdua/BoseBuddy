//! QC Buddy core: everything except the tray UI. Talks to a Bose headset over
//! the PC's Bluetooth connection (BMAP over RFCOMM), ported from BoseBuddy's
//! macOS sources. Used by the tray app and the `qcbuddy` CLI.

pub mod controller;
pub mod eq_presets;
pub mod fake;
pub mod identity;
pub mod log;
pub mod models;
pub mod modes;
pub mod peers;
pub mod platform;
pub mod protocol;
pub mod request_description;
pub mod transport;
pub mod worker;

pub use controller::{Controller, LinkState, State};
pub use worker::{Command, Worker};

#[cfg(test)]
mod controller_tests;
