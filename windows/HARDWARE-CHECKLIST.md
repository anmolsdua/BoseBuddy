# QC Buddy hardware checklist

What unit tests can't cover: the real Bluetooth link, Windows' audio
outputs, and the headset's own behaviour. Run it on the Windows test PC with
the QC45 before each release, and after changes to `core/src/transport.rs`,
`core/src/platform.rs` or the controller's device actions.

**Setup:** QC45 paired and connected to the PC, audio playing **from the PC**,
the phone as a second device. Work at the PC's own screen, or set the Remote
Desktop client to "Play on remote computer": over Remote Desktop the PC's
audio goes to the client, and the headset reports the client as the source.

Use the CLI (`windows\core`: `cargo run --release -- <command>`) or the
tray app (`windows\app`: `npm run tauri dev`). Check
`%LOCALAPPDATA%\QC Buddy\QCBuddy.log` after each section.

## Connection

| # | Check | How | Last pass |
|---|---|---|---|
| 1 | Finds the headset by vendor ID, even renamed | `qcbuddy devices` | 2026-11-25 |
| 2 | Opens the control channel while audio plays; audio keeps playing | `qcbuddy status` while playing from the PC | 2026-11-25 (audio from the MacBook) |
| 3 | Name, firmware, battery | `qcbuddy status` | 2026-11-25 |
| 4 | Attaches by itself when the headset is turned on (within a minute) | App running, headset off → on | |
| 5 | Notices the headset turning off; flyout says Not connected | App running, headset off | |
| 6 | Reconnects after the PC sleeps | Sleep, wake, open the flyout | |
| 6a | A second client (CLI while the app runs) is told the headset is busy, quickly, instead of hanging | `qcbuddy status` with the app running | 2026-11-25 |
| 6b | An unreachable headset fails within ~6 s, after one channel | `qcbuddy --address 10:00:00:00:00:99 status` | 2026-11-25 (5.4 s) |
| 6c | Every write gets the reply the core requires (`QCBUDDY_TRACE=1`, check TX/RX in the log) | Mode switch, mode edit, EQ, sidetone: 2026-11-25. Still to trace: pairing (4.8), multipoint (1.10) | partly |

## Noise control

| # | Check | How | Last pass |
|---|---|---|---|
| 7 | Full mode list, incl. Walk and Custom | `qcbuddy mode` | 2026-11-25 |
| 8 | Switch modes with the voice prompt | `qcbuddy mode walk`; flyout buttons | 2026-11-25 |
| 9 | Edit a user mode's ANC / Wind Block; survives a reconnect | `qcbuddy mode-edit walk --anc 7`, then `--wind` | |
| 10 | "+ Custom" in the free slot, switches to it | `qcbuddy custom` (needs a free slot) | |
| 10a | Wind Block on/off for a user mode from the flyout | Noise control → Wind Block | 2026-11-25 |

## Equalizer and settings

| # | Check | How | Last pass |
|---|---|---|---|
| 11 | Apply a preset; another app sees it after reopening | `qcbuddy preset warm`; flyout chips | 2026-11-25 |
| 11a | Cross-app: change EQ/mode in BoseBuddy (Mac) or the Bose app; QC Buddy shows it when the flyout opens, and BoseBuddy shows QC Buddy's change when its popover opens | Both apps | 2026-11-25 (QC Buddy side) |
| 11b | Save, rename, delete a custom preset; it shows in `qcbuddy preset` too | Flyout → Save as preset… | |
| 12 | One band at a time, incl. negative values | `qcbuddy eq-band treble -4` | |
| 13 | Sidetone Off/High/Med/Low audible on a call | Flyout → Sidetone | 2026-11-25 (set; not heard on a call) |
| 14 | Multipoint off drops to one device; on again | Flyout toggle | |

## Devices

| # | Check | How | Last pass |
|---|---|---|---|
| 15 | Device list, active-audio marker, slot count | `qcbuddy peers`; Devices card | 2026-11-25 |
| 16 | Disconnect the phone, connect it again | Flyout Disconnect / Connect | |
| 17 | Both slots full: Switch drops the other device, never this PC | Flyout Switch on a third device | |
| 18 | "Ask which" lists both and drops the chosen one | `qcbuddy connect phone --ask` | |
| 19 | Connect audio for this PC from Control only | Drop the PC's audio, then Connect audio; does Windows bring up the output? (M0 risk 3) | |
| 20 | Pairing mode: a new device can find and pair | `+ Pair new device` | |
| 21 | Power off | `qcbuddy power-off` | |

## Tray app

| # | Check | How | Last pass |
|---|---|---|---|
| 22 | Tray icon shows the battery; red at 20% or less | Look at the taskbar | 2026-11-25 |
| 23 | Click opens the flyout above the tray; click again closes it | Tray icon | 2026-11-25 (opens) |
| 24 | Closes on Esc and when clicking elsewhere | | |
| 25 | Light and dark taskbar: icon visible in both | Settings → Personalization → Colors | |
| 26 | 125% / 150% display scaling: flyout placed and sized right | Settings → Display | |
| 27 | Second launch opens the running app's flyout | Start the app again | 2026-11-25 |
