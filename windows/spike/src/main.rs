//! QC Buddy M0 spike: runs the M0 checklist from windows/PLAN.md against a
//! Bose headset over RFCOMM on Windows.
//!
//! Usage (on the Windows test PC, headset paired and playing audio):
//!   cargo run --release -- [--address 78:2B:64:5B:71:EE] [--steps 1,2,3] [--stress-seconds 60]
//!
//! Writes spike-report.txt (pass/fail per step) and spike-traffic.log (every
//! packet sent and received) to the current folder.

#[cfg_attr(not(windows), allow(dead_code))]
mod bmap;
#[cfg(windows)]
mod paired;
#[cfg(windows)]
mod rfcomm;
#[cfg(windows)]
mod spike;

fn main() {
    #[cfg(windows)]
    spike::run();
    #[cfg(not(windows))]
    {
        eprintln!("The QC Buddy spike talks to Windows Bluetooth APIs and only runs on Windows.");
        eprintln!("Elsewhere use `cargo test` (packet framing) or `cargo check --target x86_64-pc-windows-msvc`.");
        std::process::exit(1);
    }
}
