# BoseBuddy

**Control Bose QuietComfort headphones from the macOS menu bar — no phone app,
no Bose account, no cloud.**

[Website](https://bosebuddy.com/) · [Releases](https://github.com/anmolsdua/BoseBuddy/releases) · [Buy me a coffee](https://buymeacoffee.com/buddiapps)

BoseBuddy talks to the headphones through the Mac’s **Bluetooth** connection. Once the headphones are paired with macOS, BoseBuddy can manage the
other multipoint device, switch sources, change modes, adjust EQ, and read the
battery.

<p align="center">
  <img src="docs/popover.png" width="360" alt="BoseBuddy menu bar panel">
</p>

## Why

The Bose app is the only official place to manage the headphones’ paired
devices. BoseBuddy gives you that on macOS, plus a few things the phone app does
not:

- Manage the **other** connected device from the Mac.
- Choose which device to drop when both audio slots are full.
- No account, no cloud, no analytics.

**The Mac must be connected to the headphones for BoseBuddy to control them.**
BoseBuddy uses the Mac’s existing Bluetooth pairing. There is no separate
pairing step; macOS remembers the headphones and reconnects them after a
restart.

## Compatibility

Tested on **Bose QC Headphones** (`prince`, product ID `0x4075`, firmware 1.0.6).

BoseBuddy speaks BMAP over Bluetooth RFCOMM and probes channels 8, 2, and 9.
The QC Ultra, QC45, and NC700 use the same protocol family, so the core
features — battery, multipoint, paired-device management, connect/disconnect,
modes, and EQ — are expected to work. Model-specific extras such as Immersive
Audio, wind block, or a different mode/EQ layout are not exposed and may differ.

Only one Bose headphone is managed at a time. If several are paired, the app
uses the first connected one.

If a model does not connect, open an issue with
`~/Library/Logs/BoseBuddy.log`.

## Features

- Paired-device list with green connected indicators and an active-audio marker.
- **Connect / Disconnect / Switch** any device, including *This Mac*.
- **Automatic switching**: when both audio slots are in use, the non-Mac device
  is disconnected so the Mac keeps its connection. Turn on **Ask which device
  to disconnect** in the gear menu to choose instead.
- Mac audio on demand: **Connect audio** on the *This Mac* row.
- Battery and charging state.
- Noise-control modes (Quiet / Aware / custom).
- 3-band EQ and sidetone where supported.
- Multipoint on/off and pairing mode.
- Menu bar battery, launch at login, local diagnostics.
- Universal binary (Apple Silicon + Intel), macOS 14+.
- **No separate pairing step** — the app uses the Mac’s normal Bluetooth pairing.

## Install

### From a release (recommended)

1. Download **`BoseBuddy-1.1.4.dmg`** from the
   [Releases](../../releases) page.
2. Open the DMG and drag **BoseBuddy** to **Applications**. Eject the DMG.
3. **The first launch is blocked by Gatekeeper.** BoseBuddy is ad-hoc signed
   and not notarized, so macOS says it can’t verify the app. This is expected.
   Open it once with one of these:

   - **Right-click → Open** in Applications, then **Open**.
   - Double-click it, click **Done** (not **Move to Trash**), then go to
     **System Settings → Privacy & Security → Open Anyway**.
   - Or clear the quarantine flag:

     ```sh
     xattr -dr com.apple.quarantine /Applications/BoseBuddy.app
     open /Applications/BoseBuddy.app
     ```

   Full walkthrough with every dialog: **[INSTALL.md](INSTALL.md)**.

   > **Do not click “Move to Trash.”** That deletes the app. Gatekeeper is
   > asking you to confirm an unsigned download, not reporting a virus.

4. macOS asks for **Bluetooth** access. Click **Allow**. This happens once per
   install — the app uses a stable code requirement so later launches and
   rebuilds reuse the grant.
5. Connect the headphones to this Mac from the headphones: press the
   **Bluetooth button** to switch the connection to the Mac, or open the
   **Bose app** on your iPhone/iPad/Android and select this Mac.
6. Open BoseBuddy. It attaches automatically; if not, click **Connect**.
7. Click the **Bose logo** in the menu bar.

<p align="center">
  <img src="docs/onboarding.png" width="420" alt="BoseBuddy setup guide">
</p>

### First-run checklist

- [ ] BoseBuddy is in **Applications** (not the DMG or Downloads).
- [ ] Gatekeeper has been cleared once — right-click **Open**, or
      **System Settings → Privacy & Security → Open Anyway**.
- [ ] macOS Bluetooth permission is granted (System Settings → Privacy &
      Security → Bluetooth).
- [ ] The headphones are connected to this Mac (side button or Bose app).
- [ ] BoseBuddy says `Connected to: Mac + iPhone` and shows the headphones.

### Why is it unsigned?

Notarization requires a paid Apple Developer ID. BoseBuddy is a local,
open-source utility, so releases are ad-hoc signed. Gatekeeper asks once; the
full source is available and can be rebuilt with `make package`. The app has no
networking, accounts, telemetry, or cloud.

## Pairing and connecting

BoseBuddy uses the Mac’s existing Bluetooth pairing and opens the headphones’
RFCOMM channel.

- **First time:** connect the headphones to this Mac from the headphones' 
  **Bluetooth button**, or from the **Bose app** on your iPhone/iPad/Android
  and select this Mac. Open BoseBuddy; it attaches automatically.
- **After a headphone restart:** press the Bluetooth button to switch the
  connection back to the Mac, or use the Bose app. BoseBuddy reattaches; there
  is no separate re-pair.
- **If the Mac is not connected:** BoseBuddy cannot control the headphones.
  This is intentional. Connect the Mac from the headphones' Bluetooth button or
  the Bose app first.
- The headphones support two audio devices at once. The Mac must be one of
  them; the other slot is free for your phone or tablet. If both slots are
  taken by other devices, disconnect one or use the headphones’ Bluetooth
  button to switch.

The iPhone’s Bose app can hold one of the two audio connections. If the Mac
cannot connect, disconnect the phone or use the headphones’ Bluetooth button to
free a slot.

## Using BoseBuddy

The header shows what the headphones are connected to:

```
Connected to: Mac + iPhone
```

- **Mac** and **iPhone** are the devices connected to the headphones (up to
  two).
- A **green dot** in the device list means that device is connected.
- A **speaker icon** marks the device currently playing.
- The Mac row shows **Control + Audio** when BoseBuddy is connected and the
  Mac’s audio is on.

### Devices

| Action | What it does |
|---|---|
| **Connect audio** on *This Mac* | Brings the Mac’s audio online without dropping the other device. |
| **Disconnect** | Drops that device’s audio. BoseBuddy stays connected. |
| **Connect / Switch** on another device | Connects it. If both slots are full, the chooser asks what to drop. |
| **Multipoint** | Turns two-device mode on or off. |

<p align="center">
  <img src="docs/switch.png" width="360" alt="Choose which device to disconnect">
</p>

By default, switching disconnects the **non-Mac** device automatically. Turn on
**Ask which device to disconnect** in the gear menu to choose which device to
drop instead.

Unnamed stale entries in the headphones' paired-device memory are hidden by
default. Turn on **Show unnamed devices** in the gear menu to reveal them.

### Menu bar

The Bose logo shows the battery percentage. Click it to open the panel. The
gear menu has:

- Setup Guide…
- Reconnect control
- Ask which device to disconnect
- Show battery in menu bar
- Launch at login
- Open Bluetooth Settings
- Power off headphones
- Quit

## Troubleshooting

**BoseBuddy doesn’t appear in the menu bar.**
The menu bar may be full. Quit other menu bar apps, or run BoseBuddy again from
Applications. The setup guide opens automatically on first launch.

**“Headphones not connected.”**
Connect the headphones to this Mac from the headphones' **Bluetooth button** or
the **Bose app**, then click **Connect** in BoseBuddy. The app cannot control
the headphones while the Mac is disconnected.

**“Couldn’t reach the headphones.”**
Make sure they are powered on and connected to this Mac. Press the headphones'
Bluetooth button to switch to the Mac, or use the Bose app. If both audio slots
are taken, disconnect one.

**Control stops after a phone connects.**
The headphones allow two audio devices. The phone may have taken a slot; use the
headphones’ Bluetooth button to switch back to the Mac, or disconnect the phone
in BoseBuddy if it is listed.

**Mac audio doesn’t show up.**
Click **Connect audio** on *This Mac*, then choose the headphones in
**System Settings → Sound**. If it still doesn’t appear, press the Bluetooth
button on the headphones once.

**The Bluetooth prompt appears again after an update.**
macOS ties the permission to the app’s code signature. Releases use a stable
designated requirement, so this should only happen once per install. If you
build from source with a different signature, macOS may ask again.

**Gatekeeper blocks the app.**
See **[INSTALL.md](INSTALL.md)**. Right-click **Open**, or use **System
Settings → Privacy & Security → Open Anyway**. Do not click **Move to Trash**.

**Diagnostics.**
BoseBuddy writes a capped log to `~/Library/Logs/BoseBuddy.log` with connection,
peer, and audio-profile transitions.

## Build from source

Requirements: macOS 14+, Xcode command line tools (Swift 5.9+).

```sh
git clone https://github.com/anmolsdua/BoseBuddy.git
cd BoseBuddy
make build      # dist/BoseBuddy.app (universal)
make package    # dist/release/*.zip, *.dmg, source zip
make selftest   # headless BMAP check against paired headphones
```

`make install` copies the app to `/Applications` and launches it.

The build signs ad-hoc with a stable designated requirement:

```
designated => identifier "com.bosebuddy.app"
```

That keeps the Bluetooth TCC grant stable across rebuilds. The app is not
notarized.

### Repository layout

```
BoseBuddy/
  Sources/
    Bose/            BMAP codec, RFCOMM transport, controller
    UI/              menu bar panel, setup guide
    System/          CoreAudio monitor, logging
    AppDelegate.swift
    main.swift
  Resources/         Info.plist, icons
  docs/              screenshots
  build.sh           universal build
  package.sh         zip + dmg + source zip
  Makefile
```

## Protocol

BMAP packets are `[fblock, function, flags, length, payload…]`; the low nibble
of `flags` is the operator (`GET`, `SETGET`, `START`, `STATUS`, `ERROR`, …).

BoseBuddy talks BMAP over Bluetooth RFCOMM:

| Item | Value |
|---|---|
| Transport | RFCOMM (SPP) |
| Channel | 8 |
| Connection | The Mac’s existing pairing |

Device management commands:

| Action | Packet |
|---|---|
| List paired devices | `GET [4.4]` |
| Device info | `GET [4.5] <mac>` |
| Connect a device | `START [4.1] [0x00, mac…]` |
| Disconnect a device | `START [4.2] [mac…]` |
| Active source | `GET [5.1]` |
| Multipoint | `GET/SETGET [1.10]` |
| Modes | `START [31.1]` + `[31.6]` status |
| Switch mode | `START [31.3] [index, announce]` |
| Equalizer | `GET/SETGET [1.7]` |
| Battery | `GET [2.2]` |

The same `CONNECT` command is used for *This Mac* to bring up A2DP on demand.

> **Why must the Mac be connected?** BoseBuddy uses the Mac’s Bluetooth
> connection and its existing pairing. There is no separate control channel to
> set up.

## Privacy

BoseBuddy has no networking, accounts, telemetry, or cloud. Everything stays
between the Mac and the headphones. The only local file it writes is
`~/Library/Logs/BoseBuddy.log` (capped at 512 KB).

## Credits

Protocol work builds on:

- [aaronsb/bosectl](https://github.com/aaronsb/bosectl) — BMAP reverse
  engineering for QC Ultra and QuietComfort.
- [MadEyez/quietrebellion](https://github.com/MadEyez/quietrebellion) — Android
  and Windows ports.
- [Gadgetbridge](https://gadgetbridge.org/gadgets/headphones/bose/) — Bose
  multipoint management reference.

BoseBuddy is unaffiliated with Bose. Bose and QuietComfort are trademarks of Bose
Corporation.

## Support

BoseBuddy is free and open source. If it saves you a trip to the phone app, you
can [buy me a coffee](https://buymeacoffee.com/buddiapps) — completely
optional.

## License

MIT. See [LICENSE](LICENSE).
