# BoseBuddy 1.1.4

BoseBuddy attaches to the headphones over the Mac's existing Bluetooth
connection.

## What changed

- **Unnamed stale entries are hidden.** The headphones' paired-device memory can
  contain entries with no name, whose display name is just their MAC address.
  The Bose app suppresses these; BoseBuddy now does too by default.
- **Show unnamed devices** is available in the gear menu if you want to see the
  full list.
- Switching still defaults to disconnecting the **non-Mac** device, so the Mac
  keeps its slot.
- Opening BoseBuddy still makes no new Bluetooth connection.

## Install (read this first)

BoseBuddy is **not notarized**, so macOS Gatekeeper blocks the first launch.
That is expected, not a damaged download.

1. Download `BoseBuddy-1.1.4.dmg`.
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
