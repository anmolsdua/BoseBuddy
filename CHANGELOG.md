# Changelog

## 1.1.4 — 2026-11-08

- Hide unnamed MAC-only entries in the paired-device list by default. These
  are stale entries in the headphones' memory that the Bose app suppresses.
- Add **Show unnamed devices** to the gear menu for anyone who wants the full
  list.
- Switching still defaults to disconnecting the non-Mac device.

## 1.1.3 — 2026-11-08

- Switching now defaults to disconnecting the **non-Mac** device, so the Mac
  keeps its connection without a prompt.
- The device chooser is opt-in: turn on **Ask which device to disconnect** in
  the gear menu to pick which device to drop.

## 1.1.2 — 2026-11-08

- BoseBuddy now recognizes a Mac that macOS already shows connected, even when
  the IOBluetooth paired-device entry reports `connected=false`, and attaches
  over the existing link.
- Connection directions now point to the headphones' **Bluetooth button** or
  the **Bose app** on iPhone/iPad/Android, not System Settings.

## 1.1.1 — 2026-11-08

- Opening BoseBuddy no longer initiates a Bluetooth connection. It attaches
  only when the Mac is already connected, so opening the menu cannot displace
  the iPhone/iPad connection.
- If the Mac is not connected, **Connect** now asks you to connect the
  headphones in System Settings → Bluetooth first.

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
