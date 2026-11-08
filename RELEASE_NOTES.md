# BoseBuddy 1.1.2

BoseBuddy attaches to the headphones over the Mac's existing Bluetooth
connection. This release fixes the case where macOS already showed the Mac
connected but BoseBuddy still said it wasn't.

## What changed

- BoseBuddy now treats a Mac that macOS already shows connected as attachable,
  even when the IOBluetooth paired-device entry reports `connected=false`.
  It opens the control channel over the existing link instead of refusing.
- Opening BoseBuddy still makes no new Bluetooth connection, so it cannot
  displace the iPhone/iPad connection.
- Connection directions now say to use the headphones' **Bluetooth button** or
  the **Bose app** on iPhone/iPad/Android, instead of System Settings.

## Install (read this first)

BoseBuddy is **not notarized**, so macOS Gatekeeper blocks the first launch.
That is expected, not a damaged download.

1. Download `BoseBuddy-1.1.2.dmg`.
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
5. Connect the headphones to this Mac from the headphones' **Bluetooth button**
   or the **Bose app** on your phone/tablet.
6. Open BoseBuddy. It attaches automatically; if not, click **Connect**.

Full walkthrough: `INSTALL.md` in the repository.

## Notes

- The Mac must be connected to the headphones; BoseBuddy attaches to that
  existing connection.
- The headphones support two audio devices at once. If both slots are taken by
  other devices, disconnect one first.
- Diagnostics: `~/Library/Logs/BoseBuddy.log`.

MIT licensed.
