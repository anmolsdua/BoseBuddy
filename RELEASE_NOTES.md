# BoseBuddy 1.1.1

BoseBuddy attaches to the headphones over the Mac's existing Bluetooth
connection. This release fixes the case where opening the menu bar panel could
drop the iPhone/iPad connection.

## What changed

- Opening BoseBuddy no longer starts a Bluetooth connection. If the Mac is not
  already connected, the app waits and shows a Connect prompt.
- **Connect** only attaches to an existing Mac connection. If there is none,
  it asks you to connect the headphones in System Settings -> Bluetooth first.
- This means opening the menu can no longer displace the connection held by the
  iPhone or iPad.

## Install (read this first)

BoseBuddy is **not notarized**, so macOS Gatekeeper blocks the first launch.
That is expected, not a damaged download.

1. Download `BoseBuddy-1.1.1.dmg`.
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
5. Connect the headphones to this Mac in **System Settings -> Bluetooth**.
6. Open BoseBuddy and click **Connect**.

Full walkthrough: `INSTALL.md` in the repository.

## Notes

- The Mac must be connected to the headphones; BoseBuddy does not create a
  connection on its own.
- The headphones support two audio devices at once. If both slots are taken by
  other devices, disconnect one before connecting the Mac.
- Diagnostics: `~/Library/Logs/BoseBuddy.log`.

MIT licensed.
