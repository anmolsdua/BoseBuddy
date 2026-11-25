# BoseBuddy for Windows — port plan

Status: planning. Branch `feature/windows-port`.

## Goal

A Windows tray app with the same features as the macOS app: control a Bose
headset over the PC's existing Bluetooth connection, with no phone app, account
or cloud. Same look as the mockup
(<https://claude.ai/artifact/V6g4HQsg6A3ChrRgbrxFEy>): a flyout above the tray
icon with Devices, Noise control and Equalizer cards.

### Parity scope (v1)

| Feature | macOS source | Windows v1 |
|---|---|---|
| Battery in tray and header | `readBattery` | yes |
| Paired devices: connect / disconnect / switch, active-audio marker | `loadPeers`, `connectPeer`, `disconnectPeer` | yes |
| Auto-drop the non-PC device when both slots are full; "ask which" option | `connectPeer` | yes |
| Multipoint on/off, pairing mode | `setMultipoint`, `enterPairingMode` | yes |
| Noise modes incl. Walk, edit Wind Block / ANC, "+ Custom" | `NoiseModes.swift`, `updateMode`, `createCustomMode` | yes |
| Mode prompt on switch | `setMode` (announce = 1) | yes |
| EQ sliders, presets, custom presets (13-char names) | `EQPresets.swift`, `applyEQ` | yes |
| Sidetone | `setSidetone` | yes |
| "Connect audio" for this PC | `connectMacAudio` | spike first (see risks) |
| Detect renamed headsets by vendor ID 0x009E | `DeviceIdentity.swift` | yes |
| Logging (session, errors, failed requests, health) | `BarLog`, `RequestDescription.swift` | yes |
| Launch at login, onboarding | `SMAppService`, `OnboardingView` | yes (simpler onboarding) |
| Power off | `powerOff` | yes |

Out of scope for v1: switching between several Bose headsets (fork issue #4),
charging icon (QC45 doesn't report it, issue #3), firmware updates.

## Stack

- **Tauri 2**: Rust backend, web UI in WebView2, built-in tray icon.
- **UI**: TypeScript + React + Vite, styled from the mockup.
- **Protocol**: [`bose-bmap`](https://crates.io/crates/bose-bmap) 0.6.0
  (bosectl, MIT): packet framing, mode config (QC45 = "prince" 47/39-byte
  layout), battery, EQ, sidetone, multipoint, set_mode / set_cnc / set_wind,
  create/delete profile. Pin the version.
- **Windows APIs**: Microsoft's `windows` crate: Winsock `AF_BTH` RFCOMM,
  Bluetooth device enumeration, Core Audio (`IMMDeviceEnumerator`,
  `IMMNotificationClient`).
- **Tauri plugins**: `positioner` (flyout above tray), `autostart`,
  `single-instance`, `store` (EQ presets, settings), `log` (or our own logger).

## Layout

```
windows/
  PLAN.md
  src-tauri/
    Cargo.toml
    src/
      main.rs            tray, window, IPC wiring
      transport.rs       WinRfcomm: bose_bmap::Transport over AF_BTH, channel 8 → 2 → 9
      discovery.rs       paired Bose headsets (name or vendor ID 0x009E)
      audio.rs           is the headset a Windows audio endpoint? (+ change events)
      controller.rs      port of BoseController: state, request queue, polling
      peers.rs           4.x paired-device list / connect / disconnect (not in bose-bmap)
      modes.rs           Walk/Custom naming, free slot, "taken over" rule
      eq_presets.rs      presets + store
      log.rs             same lines as BarLog, %LOCALAPPDATA%\BoseBuddy\BoseBuddy.log
    tests/               fixtures: records captured from the QC45
  ui/
    src/                 React: Header, DevicesCard, NoiseCard, EqualizerCard, Footer
```

Same repo, own folder: shares issues, README, website (bosebuddy.com) and the
captured test fixtures. The macOS app is unchanged.

## Architecture

- **One request at a time.** A single worker owns the RFCOMM socket and
  processes a queue (mpsc). This carries over the macOS fix where overlapping
  requests read each other's replies.
- **GetAll (31.1) is single-flight** and accepted only when its 31.1 RESULT
  arrives; retry up to 3x, keep the last good list.
- **No new polling.** Battery/source/mode every 15 s only while the flyout is
  open; modes re-read on open; audio endpoint changes come from Core Audio
  events, not a timer.
- **IPC**: Tauri commands for actions (`set_mode`, `update_mode`,
  `create_custom_mode`, `apply_eq`, `connect_peer`, ...); one `state` event the
  UI renders from.
- **Flyout**: frameless, always-on-top window, positioned with
  `TrayBottomRight`, hidden on blur, toggled by tray left-click; right-click
  menu with Open / Quit.

## Milestones

| # | Milestone | Done when |
|---|---|---|
| M0 | **Spike** (2–4 days) | Console app on Windows connects to the QC45 on channel 8 with `bose-bmap` + our transport, prints battery, firmware, mode list, switches Quiet → Walk with the prompt. Answers the risks below. |
| M1 | Core | `controller`, `peers`, `modes`, `eq_presets`, `log` with unit tests ported from `Tests/*.swift` and the QC45 fixtures. |
| M2 | Tray + flyout | Tauri app shows live battery and modes in the mockup UI. |
| M3 | Devices | Paired list, connect / disconnect / switch, auto-drop, multipoint, pairing mode. |
| M4 | Noise + EQ | Mode editing, "+ Custom", EQ presets incl. custom names, sidetone. |
| M5 | Polish | Onboarding, autostart, settings, logging parity, light/dark. |
| M6 | Ship | NSIS/MSI installer with WebView2 bootstrapper, code signing decision, GitHub release, README section. |

## Risks to settle in M0

1. **RFCOMM while audio is connected.** Can an app open channel 8 on Windows
   while the headset is connected for A2DP/HFP, and does the audio keep
   playing?
2. **Winsock `AF_BTH` vs WinRT `RfcommDeviceService`.** WinRT finds services by
   UUID through SDP; we connect by channel number like macOS. Try Winsock first.
3. **"Connect audio" for this PC.** On macOS the app asks the headset to
   connect the Mac (CONNECT self) and macOS brings up audio. Windows may need
   to start the audio connection itself.
4. **Multipoint behaviour** with the Windows Bluetooth stack (auto-drop,
   active-source marker).
5. **WebView2 memory** (expect 50–100 MB) and whether that's acceptable for a
   tray app.
6. **SmartScreen** on an unsigned installer (same problem as Gatekeeper on
   macOS).

## Testing

- Unit tests in Rust against packets captured from the QC45 (the same records
  `Tests/NoiseModeTests.swift` uses).
- A hardware checklist per release: connect, each card's controls, switching
  devices, reconnect after sleep, log contents.
- Only one headset (QC45, fw 4.0.4) is available; other models are best-effort
  and labelled as such.

## Open questions

- App name (see below).
- Contribute the Windows transport and the 4.x peer commands back to bosectl?
- Code signing: buy a certificate, or ship unsigned with instructions like the
  macOS DMG does?

## Name

Recommended: **BoseBuddy**, released as "BoseBuddy for Windows", executable
`BoseBuddy.exe`. Same product, same website (bosebuddy.com), same README and
issues. Note: "Bose" is a third-party trademark; that's already true for the
macOS app, but it matters more if we publish to the Microsoft Store, which can
reject apps that use another company's trademark in the name. Outside the
Store (GitHub releases, bosebuddy.com) it's the same situation as today.
