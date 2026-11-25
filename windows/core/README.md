# qcbuddy-core

QC Buddy without the UI: the BMAP protocol, the controller (a port of
BoseBuddy's `BoseController.swift`), device discovery, logging and EQ presets.
The tray app (M2) depends on it. It also builds the **`qcbuddy` CLI**, which
runs every feature from a terminal, so the hardware can be tested without the
UI.

## CLI

```powershell
cd windows\core
cargo build --release
.\target\release\qcbuddy.exe              # status: battery, modes, EQ, devices
.\target\release\qcbuddy.exe --help
```

| Command | Does |
|---|---|
| `status` | Battery, firmware, modes, EQ, sidetone, multipoint, devices (default) |
| `devices` | Bose headsets paired with this PC; doesn't connect |
| `mode [NAME\|N]` | List modes, or switch (plays the voice prompt) |
| `mode-edit NAME --anc 0-10` / `--wind` | Change an editable mode |
| `custom` | Create a "Custom" mode in the free slot |
| `eq [B M T]`, `eq-band bass 4` | Show or set the equalizer |
| `preset [NAME]`, `preset-save`, `preset-rename`, `preset-delete` | EQ presets |
| `sidetone [N]`, `multipoint on\|off` | |
| `peers` | The headset's paired devices and which one is playing |
| `connect NAME [--ask]`, `disconnect NAME` | Switch devices; auto-drops a device that isn't this PC when both slots are full (`--ask` to choose) |
| `pc-audio` | Ask the headset to connect this PC's audio |
| `pairing`, `power-off` | |
| `watch [SECONDS]` | Behave like an open flyout: poll and print changes |
| `log` | Log path and its last 40 lines |

Options: `--fake` (simulated QC45, no Bluetooth needed), `--address MAC`,
`--json` (the full state the UI renders), `-v` (echo log lines).

The headset is found by name or by Bose's Bluetooth vendor ID (0x009E), so
renamed headsets work without `--address`. The log is
`%LOCALAPPDATA%\QC Buddy\QCBuddy.log`, custom EQ presets are in
`eq-presets.json` beside it.

## Layout

| Module | |
|---|---|
| `protocol` | Packet framing, errors, byte helpers (`Protocol.swift`) |
| `controller` | State, reads and actions (`BoseController.swift`) |
| `worker` | Runs the controller on one thread behind a command queue; polls while the flyout is open |
| `peers` | 4.4 / 4.5 / 5.1 parsing, auto-drop rule |
| `modes` | Mode records, Walk/Custom naming, free slot (`NoiseModes.swift`) |
| `eq_presets` | Built-in and custom presets (`EQPresets.swift`) |
| `identity` | Bose detection by name or vendor ID (`DeviceIdentity.swift`) |
| `request_description` | Plain-English failure log lines (`RequestDescription.swift`) |
| `log` | `QCBuddy.log` with rotation (`BarLog.swift`) |
| `platform`, `transport` | Windows: paired devices, Winsock RFCOMM, Core Audio outputs |
| `fake` | Simulated QC45 used by the tests and `--fake` |

## Tests

```powershell
cargo test
cargo clippy --all-targets
```

Unit tests use replies captured from the QC45 (fw 4.0.4) with device names
and addresses replaced, plus the cases from `Tests/*.swift`. Controller tests
run against the simulated headset, including failures (timeouts, lost link,
rejected connects) and the worker's polling.

`ui_fixtures.rs` writes real controller states as JSON for the flyout's tests
(`../app/src/__fixtures__`). It fails when they're out of date; regenerate with
`$env:UPDATE_FIXTURES=1; cargo test ui_fixtures`, then run `npm test` in
`../app`.

What only hardware can check is in
[`../HARDWARE-CHECKLIST.md`](../HARDWARE-CHECKLIST.md).
