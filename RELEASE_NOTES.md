# BoseBuddy 1.0.0

Control Bose QuietComfort headphones from the macOS menu bar — no phone app,
no Bose account, no cloud.

## Highlights

- **BLE control without using an audio slot.** Keep the headphones paired to
  your iPad and iPhone and still manage them from the Mac.
- **Device switching** with a chooser: when both audio slots are full, pick
  which device to disconnect. The prompt is on by default.
- **Mac audio on demand.** `Connect audio` brings this Mac online without
  touching the other device.
- **Battery, modes, EQ, sidetone**, and the active audio device at a glance.
- **First-run setup** walks through Bluetooth permission, pairing mode, and the
  one-time BLE bond.

## Install (read this first)

BoseBuddy is **not notarized**, so macOS Gatekeeper blocks the first launch.
That is expected, not a damaged download.

1. Download `BoseBuddy-1.0.0.dmg`.
2. Open it and drag **BoseBuddy** to **Applications**.
3. Open it once past Gatekeeper:
   - **Right-click BoseBuddy → Open → Open** in Applications, or
   - double-click it, click **Done** (not **Move to Trash**), then
     **System Settings → Privacy & Security → Open Anyway**, or
   - run:
     ```sh
     xattr -dr com.apple.quarantine /Applications/BoseBuddy.app
     open /Applications/BoseBuddy.app
     ```
   **Do not click “Move to Trash.”**
4. Allow Bluetooth when macOS asks (one time per install).
5. Follow the setup guide: put the headphones in pairing mode (hold the
   Bluetooth button until the light blinks) and click **Pair now**.

Full walkthrough: `INSTALL.md` in the repository.

## Notes

- One BLE central at a time: close the Bose app on your phone if BoseBuddy cannot
  find the headphones.
- The headphones support two audio devices at once. BoseBuddy shows the slots as
  `1/2 audio` or `2/2 audio`.
- If the BLE bond is lost, open **Setup Guide…** from the gear menu and re-pair
  in pairing mode.
- Diagnostics: `~/Library/Logs/BoseBuddy.log`.

MIT licensed. Protocol work builds on bosectl, Quiet Rebellion, bozo, and
Gadgetbridge.
