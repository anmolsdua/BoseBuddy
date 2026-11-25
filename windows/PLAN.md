# QC Buddy — Windows port plan

Status: planning. Branch `feature/windows-port`.

## Goal

**QC Buddy** is the Windows sibling of BoseBuddy (macOS): a tray app with the
same features. It controls a Bose headset over the PC's existing Bluetooth
connection, with no phone app, account or cloud. Same look as the mockup
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
      log.rs             same lines as BarLog, %LOCALAPPDATA%\QC Buddy\QCBuddy.log
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
| M0 | **Spike** (4–6 days) | Console app on the Windows test PC passes the M0 checklist below and answers the risks. |
| M1 | Core | `controller`, `peers`, `modes`, `eq_presets`, `log` with unit tests ported from `Tests/*.swift` and the QC45 fixtures. |
| M2 | Tray + flyout | Tauri app shows live battery and modes in the mockup UI. |
| M3 | Devices | Paired list, connect / disconnect / switch, auto-drop, multipoint, pairing mode. |
| M4 | Noise + EQ | Mode editing, "+ Custom", EQ presets incl. custom names, sidetone. |
| M5 | Polish | Onboarding, autostart, settings, logging parity, light/dark. |
| M6 | Ship on GitHub Releases | NSIS `.exe` installer published on a GitHub release with SmartScreen instructions and a README section (see Distribution). |
| M7 | Microsoft Store (follow-up) | MSIX package accepted in the Store after v1 is stable (see Distribution). |

## M0 checklist

Run on the Windows test PC (see Test setup) with the QC45 paired and playing
audio. Each line is pass/fail and goes in the M0 report.

1. Opens RFCOMM channel 8 (fall back to 2, 9) while the QC45 is connected for
   audio; audio keeps playing throughout.
2. Reads battery, firmware, product name.
3. Reads the full mode list (Quiet, Aware, Walk, slot 3) via GetAll with the
   31.1 RESULT, and switches Quiet → Walk with the voice prompt.
4. **Core feature:** reads the headset's paired-device list (4.4) with
   connected / active state, the same data as BoseBuddy's `peers:` log line.
5. **Core feature:** disconnects the iPhone (4.2) and reconnects it (4.1);
   the device list reflects both.
6. With both audio slots full, connecting a third device follows BoseBuddy's
   auto-drop rule (drops the non-PC device).
7. "Connect audio" for this PC: from a state where the PC has only the control
   link, ask the headset to connect the PC (CONNECT self) and check whether
   Windows brings up the audio endpoint.
8. Requests are serialized: a GetAll during other reads never returns a
   mixed-up reply (run it in a loop for a minute).

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

## Test setup

- **Hardware:** an Intel (x64) Windows PC with built-in Bluetooth, the QC45
  paired to it, plus the iPhone (and the Mac) as the headset's other devices
  for multipoint tests. x64 is the primary target; Windows on ARM is later.
- **Workflow:**
  1. Write code on the Mac in the `BoseBuddy-windows` worktree; push
     `feature/windows-port`.
  2. CI (GitHub Actions, `windows-latest`) builds and runs the Rust unit tests
     on every push. Runners have no Bluetooth, so CI never talks to a headset.
  3. On the Windows PC: pull, build (`cargo run` for the spike, `npm run tauri
     dev` for the app), and run the hardware checks. Logs from
     `%LOCALAPPDATA%\QC Buddy\QCBuddy.log` come back to the Mac for review.
- **Windows PC setup:** Rust (MSVC toolchain + Visual Studio Build Tools with
  the "Desktop development with C++" workload), Node LTS, Git; WebView2 is
  already on current Windows 10/11. Tauri CLI comes in with the app's npm
  dependencies.

## Testing

- Unit tests in Rust against packets captured from the QC45 (the same records
  `Tests/NoiseModeTests.swift` uses).
- A hardware checklist per release: connect, each card's controls, switching
  devices, reconnect after sleep, log contents.
- Only one headset (QC45, fw 4.0.4) is available; other models are best-effort
  and labelled as such.

## Open questions

- Contribute the Windows transport and the 4.x peer commands back to bosectl?

## Distribution

### v1: GitHub Releases (M6)

- **Installer:** Tauri's **NSIS `.exe`** (`QCBuddy_<version>_x64-setup.exe`),
  per-user install (no admin), bundles the WebView2 bootstrapper for PCs
  without it. No MSI for now.
- **Where:** a GitHub release on this repo, alongside the macOS assets, with
  SHA-256 checksums.
- **Signing:** ship **unsigned** at first, like the macOS DMG. The release
  notes and README explain SmartScreen's "Windows protected your PC" screen
  (More info → Run anyway). Revisit with Azure Trusted Signing if the warning
  hurts adoption.
- **Updates:** Tauri updater plugin, reading a `latest.json` attached to the
  GitHub release. It needs its own update-signing key (separate from code
  signing); keep the private key out of the repo, in a GitHub Actions secret.
- **Launch at login:** Tauri `autostart` plugin (registry Run key), fine
  outside MSIX.
- **Third-party notices:** ship `bose-bmap`'s MIT notice with the installer
  and in the About screen.

### Follow-up: Microsoft Store (M7)

- **Package:** **MSIX**, which the Store signs, so Store users get no
  SmartScreen warning and get automatic updates through the Store.
- **Tauri doesn't build MSIX**; package the release build with Microsoft's MSIX
  tooling (`makeappx` / MSIX Packaging Tool) in CI.
- **MSIX changes the app:**
  - Launch at login becomes a declared `StartupTask` instead of the registry
    Run key (the Run key is virtualized inside MSIX).
  - File writes under AppData are redirected, so read the log path from the
    API at runtime rather than hard-coding it.
  - Turn off the Tauri updater in the Store build; the Store updates the app.
- **Listing:** "QC Buddy — works with Bose QuietComfort headphones", no Bose
  logos or artwork; needs a Microsoft developer account, privacy statement
  (no telemetry, logs stay local) and screenshots.
- Check Microsoft's current Store, signing and account requirements before
  starting M7; they change.

Not planned: winget (can be added later as a manifest pointing at the GitHub
release), standalone MSIX outside the Store.

## Name

**QC Buddy** (decided 2026-11-25).

- Product name "QC Buddy", executable `QCBuddy.exe`, bundle identifier
  `com.qcbuddy.app`, install folder and log folder `QC Buddy`.
- Avoids "Bose" in the name, which keeps the Microsoft Store open as an
  option. Descriptions still say it works with Bose QuietComfort headphones
  (factual compatibility), with no Bose logos or artwork.
- The macOS app stays BoseBuddy. The README and bosebuddy.com link to QC Buddy
  as the Windows version.
