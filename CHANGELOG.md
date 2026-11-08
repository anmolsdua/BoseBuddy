# Changelog

## 1.0.0 — 2026-11-08

First public release.

- BLE control over the Bose `FEBE` service, so the Mac does not have to use one
  of the headphones' two audio slots.
- Paired-device list with green connected indicators, active-audio marker, and
  Connect / Disconnect / Switch actions.
- Switch chooser: when both audio slots are in use, choose which device to
  disconnect. On by default; when off, the non-Mac device is dropped.
- Mac audio on demand via `CONNECT [4.1]` for this Mac, without disturbing the
  other connection.
- Battery, charging, noise-control modes, multipoint, 3-band EQ, sidetone.
- First-run setup guide: permissions, pairing mode, one-time BLE bond.
- Universal binary (Apple Silicon + Intel), macOS 14+.
- Stable designated requirement so the Bluetooth permission grant survives
  rebuilds.
- Local diagnostics log at `~/Library/Logs/BoseBuddy.log`.
