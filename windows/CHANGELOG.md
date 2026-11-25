# QC Buddy changelog

What changed in each QC Buddy (Windows) release, newest first. The release
page for a version shows its section from this file.

## 0.1.3

- **`qcb update`**: the command-line tool says whether a newer QC Buddy is
  available; `qcb update --install` lets the tray app offer it. (The tool is
  built from source for now; it isn't in the installer yet.)
- Unknown `qcb` commands are rejected straight away instead of first trying
  to connect to the headphones.
- The first release QC Buddy 0.1.2 can install by itself.

## 0.1.2

- **Automatic updates.** QC Buddy checks for a new version when it starts
  and every 6 hours; the flyout offers it and **Install and restart**
  installs it. Updates are signed and verified first, and nothing installs
  without your click.
- **Check for updates by hand:** right-click the tray icon → **Check for
  updates**, or `QCBuddy.exe --check-for-updates`.

## 0.1.1

- The tray battery follows the taskbar's light or dark mode while QC Buddy
  runs (the white digits were invisible after switching to light mode).
- A connection that can't be made fails in about 5 seconds with a clear
  message, instead of taking up to a minute.
- If another copy of QC Buddy or the `qcbuddy` command is using the
  headphones, you're told so instead of getting timeouts.
- Replies are matched to the request that asked for them, and opening or
  closing the flyout is never lost when QC Buddy is busy.

## 0.1.0

First release: a tray app for Bose QuietComfort headphones (tested on a
QC45). Battery on the tray icon; the headphones' devices with connect,
disconnect and switch; noise modes with editing and "+ Custom"; equalizer
presets; sidetone; multipoint and pairing mode.
