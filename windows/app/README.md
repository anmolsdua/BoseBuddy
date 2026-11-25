# QC Buddy tray app

Tauri 2 app: a tray icon and a flyout above it (`src/`, React), on top of
[`../core`](../core) (all headset work). Status: M2, with the header (name,
battery, connected devices) and the Noise control card. Devices and EQ cards
come in M3/M4; the core already supports them.

## Run

Needs Node 22, the Rust **MSVC** toolchain and Visual Studio's C++ build tools
(see [`../PLAN.md`](../PLAN.md), Test setup). On a PC whose default Rust
toolchain is GNU, set an override once:

```powershell
cd windows
rustup override set stable-x86_64-pc-windows-msvc
```

Then:

```powershell
cd windows\app
npm install
npm run tauri dev                          # real headset
$env:QCBUDDY_FAKE=1; npm run tauri dev     # simulated QC45, no Bluetooth
```

Left-click the tray icon (it may be under the ^ overflow) to open the flyout;
it hides when it loses focus or on Esc. Right-click for Open / Quit. Starting
the app again opens the running one's flyout.

The log is the same as the CLI's: `%LOCALAPPDATA%\QC Buddy\QCBuddy.log`.

## How it fits together

- `src-tauri/src/main.rs`: tray, flyout placement (bottom-right of the work
  area, sized to its content), and three commands: `get_state`, `send`
  (a core `Command` as JSON) and `fit_flyout`. The core worker emits a
  `state` event after every change; the tray tooltip shows the battery.
- `src/App.tsx`: renders `state`; `src/types.ts` mirrors the core's types.
- Icons come from `icon-src/*.svg` via `npx tauri icon`; the tray icon is
  white or dark to match the taskbar theme.
