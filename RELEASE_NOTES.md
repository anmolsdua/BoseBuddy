# BoseBuddy 1.1.0

Control Bose QuietComfort headphones from the macOS menu bar - no phone app,
no Bose account, no cloud.

## What changed in 1.1.0

- **Simpler Bluetooth connection.** BoseBuddy now uses the Mac's existing
  Bluetooth pairing. There is no separate pairing step, and the headphones
  reconnect normally after a restart.
- **The Mac must be connected.** BoseBuddy manages the headphones while this Mac
  is one of their two audio connections; it is not a remote control when the Mac
  is disconnected.
- The setup guide now walks through connecting the headphones to macOS instead
  of a separate pairing step.
- The header reads `Connected to: ...`, and the Mac row shows
  `Control + Audio`.

## Highlights

- **Device switching** with a chooser: when both audio slots are full, pick
  which device to disconnect. The prompt is on by default.
- **Mac audio on demand.** `Connect audio` brings this Mac online without
  touching the other device.
- **Battery, modes, EQ, sidetone**, and the active audio device at a glance.
- **No separate re-pairing** after headphone restarts.

## Install (read this first)

BoseBuddy is **not notarized**, so macOS Gatekeeper blocks the first launch.
That is expected, not a damaged download.

1. Download `BoseBuddy-1.1.0.dmg`.
2. Open it and drag **BoseBuddy** to **Applications**.
3. Open it once past Gatekeeper:
   - **Right-click BoseBuddy -> Open -> Open** in Applications, or
   - double-click it, click **Done** (not **Move to Trash**), then
     **System Settings -> Privacy & Security -> Open Anyway**, or
   - run:
     ```sh
     xattr -dr com.apple.quarantine /Applications/BoseBuddy.app
     open /Applications/BoseBuddy.app
     ```
   **Do not click "Move to Trash."**
4. Allow Bluetooth when macOS asks (one time per install).
5. Connect the headphones to this Mac:
   - Put them in pairing mode if they are not paired with macOS yet (hold the
     Bluetooth button until the light blinks).
   - Connect them in **System Settings -> Bluetooth**.
   - Open BoseBuddy and click **Connect**.

Full walkthrough: `INSTALL.md` in the repository.

## Notes

- The headphones support two audio devices at once. The Mac must be one of
  them; the other slot is free for your phone or tablet.
- If both slots are taken by other devices, disconnect one or use the
  headphones' Bluetooth button to switch.
- The iPhone Bose app can hold one of the two audio connections. If the Mac
  cannot connect, disconnect the phone or use the headphones' button.
- Diagnostics: `~/Library/Logs/BoseBuddy.log`.

MIT licensed. Protocol work builds on bosectl, Quiet Rebellion, and
Gadgetbridge.
