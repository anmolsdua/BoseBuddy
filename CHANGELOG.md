# Changelog

## 1.1.0 — 2026-11-08

- Simplified the connection to use the Mac's existing Bluetooth pairing;
  removed the separate pairing step that had to be repeated after headphone
  restarts.
- The Mac must be connected to the headphones; BoseBuddy manages the other
  multipoint device and can bring the Mac's audio online.
- Reworked the setup guide to connect the headphones to macOS.
- The header shows `Control`; the Mac row shows `Control` or
  `Control + Audio`.
- Removed the alternate control transport, its probe, and the retry-pairing UI.

## 1.0.0 — 2026-11-08

First public release.

- A separate control path that did not use one of the headphones' two audio
  slots.
- Paired-device list with green connected indicators, active-audio marker, and
  Connect / Disconnect / Switch actions.
- Switch chooser: when both audio slots are in use, choose which device to
  disconnect. On by default; when off, the non-Mac device is dropped.
- Mac audio on demand via `CONNECT [4.1]` for this Mac, without disturbing the
  other connection.
- Battery, charging, noise-control modes, multipoint, 3-band EQ, sidetone.
- First-run setup guide: permissions, pairing, and connection.
- Universal binary (Apple Silicon + Intel), macOS 14+.
- Stable designated requirement so the Bluetooth permission grant survives
  rebuilds.
- Local diagnostics log at `~/Library/Logs/BoseBuddy.log`.
