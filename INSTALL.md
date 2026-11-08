# Installing BoseBuddy (unsigned build)

BoseBuddy is **ad-hoc signed, not notarized**. There is no Apple Developer ID
behind this build, so macOS Gatekeeper will block the first launch. That is
expected — it is not a damaged download.

Follow this once. After the app has been opened once, macOS remembers it and
later launches are normal.

## 1. Put BoseBuddy in Applications

1. Download **`BoseBuddy-1.0.0.dmg`** from the Releases page.
2. Open the DMG.
3. Drag **BoseBuddy** into the **Applications** folder.
4. Eject the DMG.

Do not run it from the DMG or from Downloads. Running from Applications avoids
macOS's temporary "app translocation" path.

## 2. Open it once past Gatekeeper

### Option A — right-click Open (recommended)

1. Open **Applications** in Finder.
2. **Right-click** (or Control-click) **BoseBuddy**.
3. Choose **Open**.
4. In the dialog, click **Open**.

### Option B — System Settings

Use this if double-clicking only offers **Done** / **Move to Trash**.

1. Double-click **BoseBuddy**. When macOS says it can't verify the app, click
   **Done**. **Do not click “Move to Trash”.**
2. Open  → **System Settings** → **Privacy & Security**.
3. Scroll down to **Security**.
4. Next to “BoseBuddy was blocked to protect your Mac”, click **Open Anyway**.
5. Authenticate with Touch ID or your password.
6. Click **Open** in the final dialog.

### Option C — Terminal

```sh
xattr -dr com.apple.quarantine /Applications/BoseBuddy.app
open /Applications/BoseBuddy.app
```

### What the dialogs mean

| Dialog | What to do |
|---|---|
| “Apple could not verify BoseBuddy is free of malware…” | Click **Done**, then use Option A or B. |
| “BoseBuddy cannot be opened because the developer cannot be verified.” | **Done** → System Settings → Privacy & Security → **Open Anyway**. |
| “BoseBuddy is damaged and should be moved to the Trash.” | **Do not move it to Trash.** Run the Terminal command in Option C. |
| “BoseBuddy was blocked to protect your Mac.” | System Settings → Privacy & Security → **Open Anyway**. |

## 3. Allow Bluetooth

macOS asks for Bluetooth access the first time BoseBuddy connects:

1. Click **Allow**.
2. If you missed it: **System Settings → Privacy & Security → Bluetooth**, and
   switch **BoseBuddy** on.

This is the normal macOS privacy prompt, not a Bose login.

## 4. Pair the headphones

1. Put the headphones in **pairing mode**: hold the Bluetooth button until the
   light blinks (you may hear “Ready to pair”).
2. Keep them near the Mac.
3. Open BoseBuddy → follow the setup guide → **Pair now**.
4. If the iPhone's Bose app is open, close it first — the headphones accept one
   BLE central at a time.

## 5. Finish

Click the **Bose logo** in the menu bar. The header shows:

```
Control: BLE  ·  Audio: Mac + iPhone
```

Use **Connect audio** on *This Mac* to bring up the Mac's audio without
dropping the other device.

## Why isn't it notarized?

Notarization requires a paid Apple Developer ID and an Apple review of every
build. BoseBuddy is a local, open-source utility, so the release is ad-hoc
signed instead. The full source is in this repository; you can build it
yourself with `make package`.

The app has no networking, accounts, telemetry, or cloud. It only talks to the
headphones over Bluetooth and writes a capped local log at
`~/Library/Logs/BoseBuddy.log`.

## If it still won't open

- Confirm the app is in **/Applications**, not in the DMG or Downloads.
- Run the Terminal command from Option C.
- Check **System Settings → Privacy & Security** for a blocked-app message.
- Re-download the DMG.
- Build from source:
  ```sh
  make package
  open dist/BoseBuddy.app
  ```
