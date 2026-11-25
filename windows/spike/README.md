# QC Buddy M0 spike

A console app that runs the M0 checklist from [`../PLAN.md`](../PLAN.md)
against a Bose headset over Bluetooth RFCOMM on Windows. It answers whether
QC Buddy can work on Windows before any app code is written.

## Run it (Windows test PC)

1. Install Rust (MSVC toolchain, with Visual Studio Build Tools' "Desktop
   development with C++" workload) and Git.
2. Pair the QC45 with the PC in Settings → Bluetooth & devices, and play some
   audio through it. Keep your phone connected to the headset too, so the
   multipoint steps have a second device.
3. Run:

   ```powershell
   git clone -b feature/windows-port https://github.com/hu13/BoseBuddy.git
   cd BoseBuddy\windows\spike
   cargo run --release
   ```

   It lists the PC's paired Bluetooth devices and picks the Bose headset by
   name. If yours has a custom name, pass its address from that list:

   ```powershell
   cargo run --release -- --address 78:2B:64:5B:71:EE
   ```

Options: `--steps 1,2,3,4` runs only some steps (step 1, connecting, always
runs); `--stress-seconds 20` shortens step 8.

## What it does

| Step | Checks | Changes the headset? |
|---|---|---|
| 1 | Opens RFCOMM channel 8 (then 2, 9); asks whether audio kept playing | No |
| 2 | Battery, firmware, name | No |
| 3 | Mode list via GetAll; switches to Walk (or Aware) with the voice prompt, then back | Briefly, restored |
| 4 | The headset's paired-device list and which device is playing | No |
| 5 | Disconnects a device you pick, then reconnects it | Yes, asks first |
| 6 | With both slots full, drops the non-PC device and connects a third | Yes, asks first; skipped unless set up |
| 7 | Drops the PC's audio, asks the headset to reconnect the PC | Yes, asks first |
| 8 | Mixed requests in a loop; checks no reply arrives for the wrong request | No |

## Bring back

Two files appear in this folder:

- `spike-report.txt`: PASS / FAIL / SKIP per step with notes.
- `spike-traffic.log`: every packet sent and received, in hex.

Send both back (or paste the report); they decide whether M1 starts and which
risks in the plan are closed. The traffic log contains device names and
Bluetooth addresses.

## Develop on the Mac

```sh
cargo test                                   # packet framing tests
rustup target add x86_64-pc-windows-msvc
cargo check --target x86_64-pc-windows-msvc  # type-check the Windows code
cargo clippy --target x86_64-pc-windows-msvc
```

It doesn't use the `bose-bmap` crate yet: 0.6.0 doesn't compile for Windows
(its RFCOMM transport uses Unix-only sockets without a platform gate).
