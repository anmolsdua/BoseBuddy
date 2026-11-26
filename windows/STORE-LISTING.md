# QC Buddy: Microsoft Store listing

What goes into Partner Center → QCBuddy → Store listings (English). Keep it
in step with the app; no Bose logos or product photos.

## Product name

QC Buddy

## Short description

Battery, noise cancelling and EQ for your Bose QuietComfort headphones, right
from the Windows taskbar.

## Description

QC Buddy puts your Bose QuietComfort headphones in the Windows taskbar. Click
the headphones icon to see the battery and change settings, with no phone
and no Bose app.

• Battery level at a glance, in the tray icon and its tooltip
• Switch noise cancelling modes, including your own custom modes and Wind Block
• Equalizer: bass, mid and treble, with presets you can save, rename and delete
• See which devices are connected, and connect, disconnect or switch between them
• Turn multipoint on or off, and put the headphones in pairing mode
• Sidetone (hear your own voice on calls)
• Follows Windows light and dark mode

Private by design: no account, no ads, no analytics. QC Buddy talks to your
headphones over your PC's Bluetooth connection, and nothing leaves your PC.

Works with Bose QuietComfort 45. Other QuietComfort models that use the same
Bluetooth protocol may work too.

QC Buddy is an independent app, not affiliated with or endorsed by Bose
Corporation. Bose and QuietComfort are trademarks of Bose Corporation.

## What's new in this version

From `CHANGELOG.md`, the version's section.

## Product features (Partner Center's short list, up to 20)

- Battery level in the taskbar
- Noise cancelling mode switching
- Custom modes and Wind Block
- Equalizer with saved presets
- Multipoint device management
- Pairing mode
- Sidetone control
- Light and dark mode

## Search terms (up to 7)

QuietComfort, headphones, noise cancelling, battery, equalizer, Bluetooth,
multipoint

## Other fields

- **Category:** Utilities & tools
- **Privacy policy URL:**
  <https://github.com/hu13/BoseBuddy/blob/main/windows/PRIVACY.md>
- **Website:** <https://github.com/hu13/BoseBuddy/tree/main/windows>
- **Support contact:** <https://github.com/hu13/BoseBuddy/issues>
- **Copyright:** © 2026 Hangkun Ung
- **Screenshots:** at least one, 1366×768 or larger: the flyout open above
  the taskbar, light and dark. Crop out other apps and personal device names.
- **System requirements:** Bluetooth; Windows 10 version 1809 or later, x64.
- **Restricted capabilities (runFullTrust) justification:** QC Buddy is a
  desktop tray app (Tauri/Win32). It talks to Bose headphones over Bluetooth
  Classic RFCOMM sockets and reads the default audio device with Windows Core
  Audio APIs, neither of which is available inside the AppContainer; it needs
  full trust to run as a packaged desktop app.
