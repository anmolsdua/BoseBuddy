# QC Buddy 0.1.0 (Windows)

The first release of **QC Buddy**, the Windows sibling of BoseBuddy: a tray
app that controls Bose QuietComfort headphones over the PC's existing
Bluetooth connection. No phone app, account or cloud.

Works with Bose QuietComfort headphones. Tested on a **QC45 (firmware
4.0.4)**; other models may work and are untested.

## What it does

- **Battery** on the tray icon and in the flyout.
- **Devices:** the headphones' paired devices, which ones are connected and
  which is playing. Connect, disconnect, and **Switch** when both audio slots
  are full (it drops a device that isn't this PC). Multipoint on/off,
  pairing mode.
- **Noise control:** switch modes (with the voice prompt), edit Wind Block
  and noise cancelling for your own modes, "+ Custom" in a free slot.
- **Equalizer:** presets, Bass/Mid/Treble, save / rename / delete your own
  presets. Sidetone.
- A command-line tool, `qcbuddy`, is not included in the installer yet; build
  it from `windows/core` if you want it.

## Install (read this first)

QC Buddy is **not code-signed yet**, so Windows SmartScreen warns on the
first run. That's expected, not a damaged download.

1. Download `QCBuddy_0.1.0_x64-setup.exe` (and, if you like,
   `SHA256SUMS.txt` to check it: `Get-FileHash QCBuddy_0.1.0_x64-setup.exe`).
2. Run it. If you see **"Windows protected your PC"**, click **More info**,
   then **Run anyway**.
3. It installs for your user only (no admin prompt) and adds **QC Buddy** to
   the Start menu.
4. Pair the headphones in **Settings → Bluetooth & devices** if you haven't,
   connect them, then start QC Buddy. Its icon appears in the tray; it may be
   under the **^** overflow, where you can drag it onto the taskbar.

Your log and EQ presets are in `%LOCALAPPDATA%\QC Buddy` and are kept if you
uninstall.

## Known limitations

- Not code-signed (see above). No automatic updates yet: download new
  versions from this page.
- No settings panel, launch at login or onboarding yet.
- Over **Remote Desktop**, Windows sends the PC's audio to the remote client,
  so QC Buddy shows this PC as "Control only".
- Custom EQ presets are stored per computer; presets saved in BoseBuddy on a
  Mac don't appear on Windows (the headphones don't store preset names).
