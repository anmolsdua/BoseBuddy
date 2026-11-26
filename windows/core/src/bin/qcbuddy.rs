//! `qcbuddy`: QC Buddy's controller from the command line, for testing every
//! feature on the Windows PC without the tray UI. `--fake` runs against a
//! simulated QC45 instead of Bluetooth.

use std::io::{self, BufRead, Write};
use std::process::ExitCode;
use std::time::Duration;

use qcbuddy_core::controller::{Controller, LinkState, State};
use qcbuddy_core::eq_presets::EqPresetStore;
use qcbuddy_core::fake::FakeHeadset;
use qcbuddy_core::log::{data_dir, default_path, Log};
use qcbuddy_core::models::Peer;
use qcbuddy_core::platform::Platform;
use qcbuddy_core::protocol::canonical_mac;

const USAGE: &str = "\
QC Buddy CLI: control a Bose headset over this PC's Bluetooth connection.

usage: qcbuddy [options] [command] [args]

commands:
  status                     battery, mode, EQ and devices (default)
  devices                    Bose headsets paired with this PC (doesn't connect)
  mode [NAME|N]              list modes, or switch (plays the voice prompt)
  mode-edit NAME|N --anc 0-10 | --wind
                             change an editable mode's noise cancelling
  custom                     create a \"Custom\" mode in the free slot and switch to it
  eq [BASS MID TREBLE]       show EQ, or set all three bands (-10..10)
  eq-band bass|mid|treble N  set one band
  preset [NAME]              list EQ presets, or apply one
  preset-save NAME           save the current EQ as a preset (13 characters max)
  preset-rename NAME NEW     rename a custom preset
  preset-delete NAME         delete a custom preset
  sidetone [N]               show or set sidetone (0 off, 1-3)
  multipoint on|off
  peers                      the headset's paired devices
  connect NAME|MAC [--ask]   connect a device; with both slots full, drops one that
                             isn't this PC (--ask: choose which)
  disconnect NAME|MAC
  pc-audio                   ask the headset to connect this PC's audio
  pairing                    put the headset in pairing mode
  power-off
  watch [SECONDS]            act like an open flyout: poll every 15 s, print changes
  log                        log file location and its last 40 lines
  update [--install]         is there a newer QC Buddy? (--install: the tray app
                             checks and offers Install and restart; the Store
                             copy opens its Store page)

options:
  --fake                     use a simulated QC45 instead of Bluetooth
  --address MAC              headset to use (default: the one connected to this PC)
  --json                     print the full state as JSON after the command
  -v, --verbose              echo log lines to the console
  -h, --help
";

struct Options {
    fake: bool,
    address: Option<String>,
    json: bool,
    verbose: bool,
    command: String,
    args: Vec<String>,
}

fn parse_options() -> Result<Options, String> {
    let mut o = Options { fake: false, address: None, json: false, verbose: false, command: String::new(), args: Vec::new() };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--fake" => o.fake = true,
            "--json" => o.json = true,
            "-v" | "--verbose" => o.verbose = true,
            "--address" => o.address = Some(it.next().ok_or("--address needs a value")?),
            "-h" | "--help" => o.command = "help".into(),
            _ if o.command.is_empty() && !arg.starts_with("--") => o.command = arg,
            _ => o.args.push(arg),
        }
    }
    if o.command.is_empty() {
        o.command = "status".into();
    }
    Ok(o)
}

fn main() -> ExitCode {
    let o = match parse_options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if o.command == "help" {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if o.command == "log" {
        return show_log(o.fake);
    }
    if matches!(o.command.as_str(), "update" | "check-update" | "check-updates")
        || (o.command == "check" && o.args.first().is_some_and(|a| a.starts_with("update")))
    {
        return update(o.args.iter().any(|a| a == "--install"));
    }
    // Reject typos before spending seconds connecting to the headset.
    if !HEADSET_COMMANDS.contains(&o.command.as_str()) && o.command != "devices" {
        eprintln!("unknown command \"{}\" (see --help)", o.command);
        return ExitCode::from(2);
    }

    let log_path = if o.fake { data_dir().join("QCBuddy-fake.log") } else { default_path() };
    let log = if o.verbose { Log::file_and_stderr(&log_path) } else { Log::file(&log_path) };
    let Some(mut platform) = platform(o.fake) else {
        eprintln!("Bluetooth control needs Windows; use --fake to try the simulated headset.");
        return ExitCode::from(2);
    };

    if o.command == "devices" {
        let found = platform.paired_headphones();
        if found.is_empty() {
            println!("No Bose headsets are paired with this PC.");
        }
        for h in found {
            println!("  {} {:<28} {}", dot(h.connected), h.name, h.address);
        }
        return ExitCode::SUCCESS;
    }

    let mut c = Controller::new(platform, log);
    connect(&mut c, o.address.as_deref());
    if !c.state().link.is_connected() {
        let reason = match &c.state().link {
            LinkState::Failed(m) => m.clone(),
            _ => "No Bose headset is connected to this PC. Turn the headphones on, or pass --address.".into(),
        };
        eprintln!("Not connected: {reason}");
        return ExitCode::FAILURE;
    }

    let mut presets = if o.fake { EqPresetStore::in_memory() } else { EqPresetStore::open(data_dir().join("eq-presets.json")) };
    let result = run(&mut c, &mut presets, &o.command, &o.args);

    if o.json {
        println!("{}", serde_json::to_string_pretty(c.state()).unwrap_or_default());
    }
    match result {
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
        Ok(()) => match &c.state().error {
            Some(error) => {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            }
            None => ExitCode::SUCCESS,
        },
    }
}

fn platform(fake: bool) -> Option<Box<dyn Platform>> {
    if fake {
        return Some(Box::new(FakeHeadset::default()));
    }
    #[cfg(windows)]
    {
        Some(Box::new(qcbuddy_core::platform::WindowsPlatform))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn connect(c: &mut Controller, address: Option<&str>) {
    if let Some(address) = address {
        c.refresh_paired_devices();
        c.connect(Some(&canonical_mac(address)));
        return;
    }
    // Like the app's launch: attach to a headset connected to this PC. From
    // the CLI the user asked for it, so also try the first paired one.
    c.start();
    if !c.state().link.is_connected() {
        if let Some(first) = c.state().headphones.first().map(|h| h.address.clone()) {
            c.clear_error();
            c.disconnect();
            c.connect(Some(&first));
        }
    }
}

/// Commands that talk to the headset (everything `run` handles).
const HEADSET_COMMANDS: &[&str] = &[
    "status", "mode", "mode-edit", "custom", "eq", "eq-band", "preset", "preset-save", "preset-rename",
    "preset-delete", "sidetone", "multipoint", "peers", "connect", "disconnect", "pc-audio", "pairing",
    "power-off", "watch",
];

/// `qcb update`: read the feed installed copies read and say whether there's
/// a newer QC Buddy. Uses Windows' own curl.exe, so the CLI needs no HTTPS
/// code of its own. With --install, the tray app checks and offers it.
fn update(install: bool) -> ExitCode {
    use qcbuddy_core::update_feed::{feed_version, verdict, Verdict, CURRENT_VERSION, WINDOWS_FEED_URL};
    // The Store copy updates through the Store, which may not have the
    // latest GitHub release yet: don't compare it with the GitHub feed.
    if qcbuddy_core::package::is_packaged() {
        println!("This is QC Buddy {CURRENT_VERSION} from the Microsoft Store, which keeps it up to date.");
        if install {
            if qcbuddy_core::package::open_store_page() {
                println!("Opened QC Buddy's Store page.");
            } else {
                eprintln!("Couldn't open the Microsoft Store.");
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }
    let fetched = std::process::Command::new("curl.exe")
        .args(["--silent", "--show-error", "--fail", "--location", "--max-time", "15", WINDOWS_FEED_URL])
        .output();
    let feed = match fetched {
        Err(e) => {
            eprintln!("Couldn't check for updates: curl.exe isn't available ({e}).");
            return ExitCode::FAILURE;
        }
        Ok(out) if !out.status.success() => {
            let reason = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if reason.contains("404") {
                println!("No QC Buddy update has been published yet (this is {CURRENT_VERSION}).");
                return ExitCode::SUCCESS;
            }
            eprintln!("Couldn't check for updates: {reason}");
            return ExitCode::FAILURE;
        }
        Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
    };
    let latest = match feed_version(&feed) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Couldn't check for updates: {e}.");
            return ExitCode::FAILURE;
        }
    };
    match verdict(CURRENT_VERSION, &latest) {
        Verdict::UpToDate => println!("QC Buddy {CURRENT_VERSION} is up to date."),
        Verdict::Ahead(v) => println!("This is QC Buddy {CURRENT_VERSION}, newer than the published {v}."),
        Verdict::Newer(v) => {
            println!("QC Buddy {v} is available (this is {CURRENT_VERSION}).");
            if !install {
                println!("Run `qcb update --install`, or right-click the tray icon → Check for updates.");
            }
        }
    }
    if install {
        // The tray app holds the updater (and its signature check): let it offer the update.
        let app = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("QCBuddy.exe")));
        match app.filter(|p| p.exists()) {
            Some(app) => match std::process::Command::new(&app).arg("--check-for-updates").spawn() {
                Ok(_) => println!("Opened QC Buddy to check and install: click \"Install and restart\" in its flyout."),
                Err(e) => eprintln!("Couldn't start {}: {e}", app.display()),
            },
            None => println!("QC Buddy (QCBuddy.exe) isn't installed next to qcb; install it from the releases page."),
        }
    }
    ExitCode::SUCCESS
}

fn run(c: &mut Controller, presets: &mut EqPresetStore, command: &str, args: &[String]) -> Result<(), String> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    match command {
        "status" => print_status(c.state(), presets),
        "mode" => match arg(0) {
            None => print_modes(c.state()),
            Some(which) => {
                let index = find_mode(c.state(), which)?;
                c.set_mode(index);
                print_modes(c.state());
            }
        },
        "mode-edit" => {
            let index = find_mode(c.state(), arg(0).ok_or("which mode?")?)?;
            let mode = c.state().modes.iter().find(|m| m.index == index).cloned().ok_or("no such mode")?;
            if !mode.editable {
                return Err(format!("{} can't be edited (only user modes can).", mode.name));
            }
            if args.iter().any(|a| a == "--wind") {
                c.update_mode(index, 0, true);
            } else {
                let anc: u8 = flag_value(args, "--anc").ok_or("use --anc 0-10 or --wind")?;
                if anc > 10 {
                    return Err("--anc is 0-10 (10 = full noise cancelling)".into());
                }
                c.update_mode(index, 10 - anc, false);
            }
            print_modes(c.state());
        }
        "custom" => {
            if c.state().free_mode_slot.is_none() {
                return Err("No free mode slot. Delete a mode in the Bose app first.".into());
            }
            c.create_custom_mode();
            print_modes(c.state());
        }
        "eq" => {
            if !args.is_empty() {
                let values: Vec<i32> = args.iter().map(|a| a.parse().map_err(|_| format!("not a number: {a}"))).collect::<Result<_, _>>()?;
                if values.len() != c.state().eq_bands.len() {
                    return Err(format!("give {} values (bass mid treble)", c.state().eq_bands.len()));
                }
                c.apply_eq(&values);
            }
            print_eq(c.state(), presets);
        }
        "eq-band" => {
            let name = arg(0).ok_or("which band? bass, mid or treble")?.to_lowercase();
            let band = c.state().eq_bands.iter().find(|b| b.name.to_lowercase() == name).cloned().ok_or(format!("no band {name}"))?;
            let value: i32 = arg(1).and_then(|v| v.parse().ok()).ok_or("give a value, e.g. 4 or -3")?;
            c.set_eq(band.band_id, value);
            print_eq(c.state(), presets);
        }
        "preset" => match arg(0) {
            None => {
                let current = presets.preset_matching(&c.state().eq_values()).map(|p| p.id);
                for p in presets.all() {
                    let mark = if Some(&p.id) == current.as_ref() { "*" } else { " " };
                    let kind = if p.is_built_in { "" } else { "  (custom)" };
                    println!(" {mark} {:<14} {:?}{kind}", p.name, p.values);
                }
            }
            Some(name) => {
                let preset = find_preset(presets, name)?;
                c.apply_eq(&preset.values);
                print_eq(c.state(), presets);
            }
        },
        "preset-save" => {
            let name = args.join(" ");
            let saved = presets.save(&name, &c.state().eq_values()).ok_or(format!("\"{name}\" is blank or already used"))?;
            println!("Saved {} {:?}", saved.name, saved.values);
        }
        "preset-rename" => {
            let preset = find_preset(presets, arg(0).ok_or("which preset?")?)?;
            let new = args[1..].join(" ");
            if preset.is_built_in || !presets.rename(&preset.id, &new) {
                return Err(format!("can't rename {} to \"{new}\"", preset.name));
            }
            println!("Renamed.");
        }
        "preset-delete" => {
            let preset = find_preset(presets, &args.join(" "))?;
            if preset.is_built_in || !presets.delete(&preset.id) {
                return Err(format!("{} is built in", preset.name));
            }
            println!("Deleted {}.", preset.name);
        }
        "sidetone" => {
            if let Some(level) = arg(0) {
                let level: u8 = level.parse().map_err(|_| "sidetone is 0-3")?;
                c.set_sidetone(level);
            }
            println!("Sidetone {}", c.state().sidetone);
        }
        "multipoint" => {
            let on = match arg(0) {
                Some("on") => true,
                Some("off") => false,
                _ => return Err("multipoint on|off".into()),
            };
            c.set_multipoint(on);
            println!("Multipoint {}", if c.state().multipoint { "on" } else { "off" });
        }
        "peers" => print_peers(c.state()),
        "connect" => {
            let ask = args.iter().any(|a| a == "--ask");
            let query: Vec<&str> = args.iter().map(String::as_str).filter(|a| *a != "--ask").collect();
            let peer = find_peer(c.state(), &query.join(" "))?;
            c.set_ask_before_switch(ask);
            c.connect_peer(&peer.mac, false);
            if let Some(pending) = c.state().pending_switch.clone() {
                println!("Both audio slots are full. Drop which device?");
                let names: Vec<String> = pending.candidates.iter().map(|p| p.display_name().to_string()).collect();
                match choose(&names) {
                    Some(i) => c.confirm_pending_switch(&pending.candidates[i].mac),
                    None => {
                        c.cancel_pending_switch();
                        println!("Cancelled.");
                    }
                }
            }
            print_peers(c.state());
        }
        "disconnect" => {
            let peer = find_peer(c.state(), &args.join(" "))?;
            c.disconnect_peer(&peer.mac);
            print_peers(c.state());
        }
        "pc-audio" => {
            c.connect_pc_audio(false);
            println!("This PC's audio: {}", if c.state().pc_audio { "connected" } else { "not connected" });
        }
        "pairing" => {
            c.enter_pairing_mode();
            if c.state().pairing_mode {
                println!("Pairing mode on: pair the new device from its Bluetooth settings.");
            }
        }
        "power-off" => {
            c.power_off();
            println!("Power off sent.");
        }
        "watch" => {
            let seconds: u64 = arg(0).and_then(|s| s.parse().ok()).unwrap_or(15);
            c.flyout_opened();
            let mut last = summary(c.state());
            println!("{last}");
            loop {
                std::thread::sleep(Duration::from_secs(seconds));
                c.refresh_pc_audio();
                c.poll();
                let now = summary(c.state());
                if now != last {
                    println!("{now}");
                    last = now;
                }
                if !c.state().link.is_connected() {
                    return Err("connection lost".into());
                }
            }
        }
        other => return Err(format!("unknown command \"{other}\" (see --help)")),
    }
    Ok(())
}

fn show_log(fake: bool) -> ExitCode {
    let path = if fake { data_dir().join("QCBuddy-fake.log") } else { default_path() };
    println!("{}", path.display());
    if let Ok(text) = std::fs::read_to_string(&path) {
        let lines: Vec<&str> = text.lines().collect();
        for line in &lines[lines.len().saturating_sub(40)..] {
            println!("{line}");
        }
    }
    ExitCode::SUCCESS
}

// MARK: - Lookups

fn find_mode(s: &State, which: &str) -> Result<u8, String> {
    if let Ok(n) = which.parse::<u8>() {
        if s.modes.iter().any(|m| m.index == n) {
            return Ok(n);
        }
    }
    s.modes
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(which))
        .map(|m| m.index)
        .ok_or_else(|| format!("no mode \"{which}\" (have: {})", s.modes.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ")))
}

fn find_preset(presets: &EqPresetStore, name: &str) -> Result<qcbuddy_core::eq_presets::EqPreset, String> {
    presets.all().into_iter().find(|p| p.name.eq_ignore_ascii_case(name.trim())).ok_or_else(|| format!("no preset \"{name}\""))
}

fn find_peer(s: &State, query: &str) -> Result<Peer, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("which device? give part of its name or its MAC".into());
    }
    let mac = canonical_mac(query);
    if let Some(p) = s.peers.iter().find(|p| p.mac == mac) {
        return Ok(p.clone());
    }
    let lower = query.to_lowercase();
    let matches: Vec<&Peer> = s.peers.iter().filter(|p| p.display_name().to_lowercase().contains(&lower)).collect();
    match matches.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(format!("no device matching \"{query}\"")),
        many => Err(format!(
            "\"{query}\" matches {}; be more specific",
            many.iter().map(|p| p.display_name()).collect::<Vec<_>>().join(", ")
        )),
    }
}

fn flag_value<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1)?.parse().ok()
}

fn choose(options: &[String]) -> Option<usize> {
    for (i, o) in options.iter().enumerate() {
        println!("  {}) {o}", i + 1);
    }
    print!("Number, or Enter to cancel: ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).ok()?;
    line.trim().parse::<usize>().ok().filter(|n| (1..=options.len()).contains(n)).map(|n| n - 1)
}

// MARK: - Output

fn dot(on: bool) -> &'static str {
    if on {
        "●"
    } else {
        "○"
    }
}

fn print_status(s: &State, presets: &EqPresetStore) {
    println!(
        "{} · channel {} · firmware {}",
        s.device_name,
        s.channel,
        if s.firmware.is_empty() { "?" } else { &s.firmware }
    );
    println!(
        "Battery    {}{}",
        s.battery.map(|b| format!("{b}%")).unwrap_or_else(|| "?".into()),
        if s.charging { " (charging)" } else { "" }
    );
    println!("PC audio   {}", if s.pc_audio { "yes" } else { "no (control link only)" });
    print_modes(s);
    print_eq(s, presets);
    println!(
        "Sidetone   {}    Multipoint {}",
        if s.supports_sidetone { s.sidetone.to_string() } else { "n/a".into() },
        if s.multipoint { "on" } else { "off" }
    );
    print_peers(s);
}

fn print_modes(s: &State) {
    let list: Vec<String> = s
        .modes
        .iter()
        .map(|m| {
            let mark = if Some(m.index) == s.mode_index { "*" } else { "" };
            format!("{} {}{mark} ({})", m.index, m.name, m.anc_label())
        })
        .collect();
    println!(
        "Modes      {}{}",
        list.join(" · "),
        s.free_mode_slot.map(|n| format!("  [free slot {n}]")).unwrap_or_default()
    );
}

fn print_eq(s: &State, presets: &EqPresetStore) {
    let bands: Vec<String> = s.eq_bands.iter().map(|b| format!("{} {:+}", b.name, b.value)).collect();
    let preset = presets.preset_matching(&s.eq_values()).map(|p| format!("  (preset {})", p.name)).unwrap_or_default();
    println!("EQ         {}{preset}", if bands.is_empty() { "n/a".into() } else { bands.join(" · ") });
}

fn print_peers(s: &State) {
    println!("Devices");
    for p in &s.peers {
        let status = if p.is_local && !s.pc_audio && p.connected { "This PC · Control only" } else { p.status_text() };
        println!("  {} {:<30} {}  {status}", dot(p.connected || (p.is_local && s.pc_audio)), p.display_name(), p.mac);
    }
}

fn summary(s: &State) -> String {
    let mode = s.current_mode().map(|m| format!("{} ({})", m.name, m.anc_label())).unwrap_or_else(|| "?".into());
    let devices: Vec<String> = s
        .peers
        .iter()
        .filter(|p| p.connected)
        .map(|p| format!("{}{}", p.display_name(), if p.active { " ♪" } else { "" }))
        .collect();
    format!(
        "battery {} · {mode} · PC audio {} · {}",
        s.battery.map(|b| format!("{b}%")).unwrap_or_else(|| "?".into()),
        if s.pc_audio { "yes" } else { "no" },
        devices.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use qcbuddy_core::fake::{peer_mac, FakeHeadset};

    fn state() -> State {
        let mut c = Controller::new(Box::new(FakeHeadset::default()), Log::memory());
        c.start();
        c.state().clone()
    }

    #[test]
    fn devices_by_mac_or_unique_name_part() {
        let s = state();
        assert_eq!(find_peer(&s, "iphone").unwrap().mac, peer_mac(3));
        assert_eq!(find_peer(&s, "10-00-00-00-00-04").unwrap().mac, peer_mac(4));
        assert_eq!(find_peer(&s, " MacBook Pro ").unwrap().mac, peer_mac(1));
        let ambiguous = find_peer(&s, "macbook").unwrap_err();
        assert!(ambiguous.contains("matches") && ambiguous.contains("be more specific"), "{ambiguous}");
        assert!(find_peer(&s, "pixel").unwrap_err().starts_with("no device matching"));
        assert!(find_peer(&s, "  ").is_err());
    }

    #[test]
    fn modes_by_index_or_name() {
        let s = state();
        assert_eq!(find_mode(&s, "walk"), Ok(2));
        assert_eq!(find_mode(&s, "1"), Ok(1));
        let missing = find_mode(&s, "9").unwrap_err();
        assert_eq!(missing, "no mode \"9\" (have: Quiet, Aware, Walk)");
    }

    #[test]
    fn presets_by_name() {
        let store = EqPresetStore::in_memory();
        assert_eq!(find_preset(&store, " WARM ").unwrap().values, [4, 0, -3]);
        assert!(find_preset(&store, "beebeebop").is_err());
    }

    #[test]
    fn every_headset_command_is_known_before_connecting() {
        // If `run` handles a command, HEADSET_COMMANDS must list it, or the
        // CLI would reject it as a typo without connecting.
        let headset = FakeHeadset::default();
        let mut c = Controller::new(Box::new(headset), Log::memory());
        c.start();
        let mut presets = EqPresetStore::in_memory();
        let err = run(&mut c, &mut presets, "frobnicate", &[]).unwrap_err();
        assert!(err.starts_with("unknown command"));
        // `watch` polls forever; it is matched in `run` like the rest.
        for command in HEADSET_COMMANDS.iter().filter(|c| **c != "watch") {
            if let Err(e) = run(&mut c, &mut presets, command, &[]) {
                assert!(!e.starts_with("unknown command"), "{command} isn't handled by run");
            }
        }
    }

    #[test]
    fn flag_values() {
        let args: Vec<String> = ["walk", "--anc", "7"].iter().map(|s| s.to_string()).collect();
        assert_eq!(flag_value::<u8>(&args, "--anc"), Some(7));
        assert_eq!(flag_value::<u8>(&args, "--wind"), None);
        let bad: Vec<String> = ["--anc", "x"].iter().map(|s| s.to_string()).collect();
        assert_eq!(flag_value::<u8>(&bad, "--anc"), None);
    }

    #[test]
    fn commands_against_the_simulated_headset() {
        let headset = FakeHeadset::default();
        let mut c = Controller::new(Box::new(headset.clone()), Log::memory());
        c.start();
        let mut presets = EqPresetStore::in_memory();
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        run(&mut c, &mut presets, "mode", &args(&["walk"])).unwrap();
        assert_eq!(headset.state().mode_index, 2);
        run(&mut c, &mut presets, "mode-edit", &args(&["walk", "--anc", "7"])).unwrap();
        assert_eq!(headset.state().records[2][42], 3, "ANC 7 is raw CNC 3");
        assert!(run(&mut c, &mut presets, "mode-edit", &args(&["quiet", "--anc", "5"])).unwrap_err().contains("can't be edited"));
        assert!(run(&mut c, &mut presets, "mode-edit", &args(&["walk", "--anc", "11"])).is_err());
        run(&mut c, &mut presets, "eq", &args(&["4", "0", "-3"])).unwrap();
        assert_eq!(c.state().eq_values(), [4, 0, -3]);
        assert!(run(&mut c, &mut presets, "eq", &args(&["4", "0"])).unwrap_err().contains("give 3 values"));
        run(&mut c, &mut presets, "preset-save", &args(&["Late", "night"])).unwrap();
        assert_eq!(presets.custom[0].name, "Late night");
        assert!(run(&mut c, &mut presets, "preset-delete", &args(&["Flat"])).unwrap_err().contains("built in"));
        run(&mut c, &mut presets, "sidetone", &args(&["2"])).unwrap();
        run(&mut c, &mut presets, "multipoint", &args(&["off"])).unwrap();
        assert!(!headset.state().multipoint);
        assert!(run(&mut c, &mut presets, "multipoint", &args(&["maybe"])).is_err());
        run(&mut c, &mut presets, "connect", &args(&["iphone"])).unwrap();
        assert!(headset.state().peer(&peer_mac(3)).unwrap().connected);
        assert!(run(&mut c, &mut presets, "frobnicate", &[]).unwrap_err().starts_with("unknown command"));
    }
}
