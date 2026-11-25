//! The M0 checklist (windows/PLAN.md). Each step records PASS / FAIL / SKIP
//! with a note; anything that changes the headset's state asks first.

use std::fs::File;
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};

use crate::bmap::{self, Op, Packet};
use crate::paired;
use crate::rfcomm::Rfcomm;

const CHANNELS: [u8; 3] = [8, 2, 9];

#[derive(Clone, Copy, PartialEq)]
enum Outcome {
    Pass,
    Fail,
    Skip,
}

struct StepResult {
    number: u8,
    title: &'static str,
    outcome: Outcome,
    note: String,
}

struct Peer {
    mac: String,
    name: String,
    connected: bool,
    local: bool,
}

struct Session {
    link: Rfcomm,
    address: u64,
    traffic: File,
}

impl Session {
    fn log(&mut self, line: &str) {
        let _ = writeln!(self.traffic, "{:>8.3}s {line}", START.with(|s| s.elapsed().as_secs_f32()));
    }

    /// One request; every byte in and out goes to the traffic log.
    fn request(&mut self, fblock: u8, function: u8, op: Op, payload: &[u8], drain: bool,
               timeout: Duration) -> Result<Vec<Packet>, String> {
        let pkt = bmap::encode(fblock, function, op, payload);
        self.log(&format!("TX {fblock}.{function} {op:?} [{}]", bmap::hex(&pkt)));
        match self.link.exchange(&pkt, drain, timeout) {
            Ok((data, stale)) => {
                if !stale.is_empty() {
                    self.log(&format!("   stale bytes discarded [{}]", bmap::hex(&stale)));
                }
                let packets = bmap::parse_all(&data);
                for p in &packets {
                    self.log(&format!("RX {}.{} {:?} [{}]", p.fblock, p.function, p.op, bmap::hex(&p.payload)));
                }
                Ok(packets)
            }
            Err(e) => {
                self.log(&format!("   failed: {e}"));
                Err(e)
            }
        }
    }

    /// GET (or another single-reply op) that must answer from the same address.
    fn single(&mut self, fblock: u8, function: u8, op: Op, payload: &[u8], timeout: Duration)
        -> Result<Packet, String> {
        let packets = self.request(fblock, function, op, payload, false, timeout)?;
        let reply = packets.iter().find(|p| p.is(fblock, function)).cloned().ok_or_else(|| {
            match packets.first() {
                Some(p) => format!("reply came from {}.{}, expected {fblock}.{function}", p.fblock, p.function),
                None => "no complete packet in the reply".to_string(),
            }
        })?;
        if reply.op == Op::Error {
            let code = reply.payload.first().copied().unwrap_or(0);
            return Err(format!("device error {code} for {fblock}.{function}"));
        }
        Ok(reply)
    }

    fn get(&mut self, fblock: u8, function: u8, payload: &[u8]) -> Result<Packet, String> {
        self.single(fblock, function, Op::Get, payload, Duration::from_millis(2800))
    }

    fn send_only(&mut self, fblock: u8, function: u8, op: Op, payload: &[u8]) -> Result<(), String> {
        let pkt = bmap::encode(fblock, function, op, payload);
        self.log(&format!("TX {fblock}.{function} {op:?} [{}] (no reply expected)", bmap::hex(&pkt)));
        self.link.send_only(&pkt)
    }

    fn reopen(&mut self) -> Result<(), String> {
        self.log("reopening RFCOMM link");
        self.link = Rfcomm::open(self.address, &[self.link.channel])?;
        Ok(())
    }

    // ── Peers (4.x), as in BoseBuddy's loadPeers / readPeerInfo ──────────────

    fn peers(&mut self) -> Result<Vec<Peer>, String> {
        let list = self.get(4, 4, &[])?;
        let mut macs = Vec::new();
        let mut i = 1;
        while i + 6 <= list.payload.len() {
            let chunk = &list.payload[i..i + 6];
            if chunk.iter().any(|&b| b != 0) {
                macs.push(bmap::format_mac(chunk));
            }
            i += 6;
        }
        let mut peers = Vec::new();
        for mac in macs {
            peers.push(self.peer_info(&mac).unwrap_or(Peer {
                name: mac.clone(),
                mac,
                connected: false,
                local: false,
            }));
        }
        Ok(peers)
    }

    fn peer_info(&mut self, mac: &str) -> Result<Peer, String> {
        let bytes = bmap::mac_bytes(mac).ok_or("bad MAC")?;
        let reply = self.get(4, 5, &bytes)?;
        let b = &reply.payload;
        if b.len() < 7 {
            return Err("short 4.5 reply".into());
        }
        let bose = b[6] & 0x04 != 0;
        let name_start = if bose { 10 } else { 9 };
        let name = if name_start < b.len() { bmap::string(&b[name_start..]) } else { String::new() };
        Ok(Peer {
            mac: mac.to_string(),
            name: if name.is_empty() { mac.to_string() } else { name },
            connected: b[6] & 0x01 != 0,
            local: b[6] & 0x02 != 0,
        })
    }

    fn active_source(&mut self) -> Option<String> {
        let reply = self.get(5, 1, &[]).ok()?;
        let b = &reply.payload;
        (b.len() >= 9 && b[2] == 1).then(|| bmap::format_mac(&b[3..9]))
    }

    /// Poll 4.5 until `connected` matches, like BoseBuddy's performConnect/Disconnect.
    fn wait_connected(&mut self, mac: &str, want: bool, tries: u32, every: Duration) -> bool {
        for _ in 0..tries {
            std::thread::sleep(every);
            if let Ok(p) = self.peer_info(mac) {
                if p.connected == want {
                    return true;
                }
            }
        }
        false
    }

    fn disconnect(&mut self, mac: &str) -> Result<bool, String> {
        let bytes = bmap::mac_bytes(mac).ok_or("bad MAC")?;
        self.send_only(4, 2, Op::Start, &bytes)?;
        Ok(self.wait_connected(mac, false, 12, Duration::from_millis(400)))
    }

    fn connect(&mut self, mac: &str) -> Result<bool, String> {
        let bytes = bmap::mac_bytes(mac).ok_or("bad MAC")?;
        let mut payload = vec![0x00];
        payload.extend_from_slice(&bytes);
        let packets = self.request(4, 1, Op::Start, &payload, false, Duration::from_secs(4))?;
        if let Some(err) = packets.iter().find(|p| p.is(4, 1) && p.op == Op::Error) {
            return Err(format!("device error {} for 4.1", err.payload.first().copied().unwrap_or(0)));
        }
        Ok(self.wait_connected(mac, true, 14, Duration::from_millis(500)))
    }
}

thread_local! {
    static START: Instant = Instant::now();
}

// ── Console helpers ──────────────────────────────────────────────────────────

fn ask(question: &str) -> bool {
    print!("{question} [y/n] ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().lock().read_line(&mut line);
    line.trim().eq_ignore_ascii_case("y")
}

fn choose(prompt: &str, options: &[String]) -> Option<usize> {
    if options.is_empty() {
        return None;
    }
    for (i, o) in options.iter().enumerate() {
        println!("  {}) {o}", i + 1);
    }
    print!("{prompt} (number, or Enter to skip): ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().lock().read_line(&mut line);
    line.trim().parse::<usize>().ok().filter(|n| *n >= 1 && *n <= options.len()).map(|n| n - 1)
}

fn print_peers(peers: &[Peer], active: Option<&str>) {
    for p in peers {
        println!(
            "    {} {:<28} {}{}{}",
            if p.connected { "●" } else { "○" },
            p.name,
            p.mac,
            if p.local { "  (this PC)" } else { "" },
            if Some(p.mac.as_str()) == active { "  ← playing" } else { "" }
        );
    }
}

// ── Steps ────────────────────────────────────────────────────────────────────

fn step2_reads(s: &mut Session) -> (Outcome, String) {
    let init = s.get(0, 1, &[]).map(|p| bmap::hex(&p.payload)).unwrap_or_else(|e| e);
    let battery = s.get(2, 2, &[]).map(|p| p.payload.first().copied());
    let firmware = s.get(0, 5, &[]).map(|p| bmap::string(&p.payload));
    let name = s.get(1, 2, &[]).map(|p| bmap::string(&p.payload));
    println!("    init 0.1: {init}");
    println!("    battery:  {:?}", battery);
    println!("    firmware: {:?}", firmware);
    println!("    name:     {:?}", name);
    match (battery, firmware) {
        (Ok(Some(b)), Ok(f)) => (Outcome::Pass, format!("battery {b}%, firmware {f}, name {:?}", name.ok())),
        (b, f) => (Outcome::Fail, format!("battery {b:?}, firmware {f:?}")),
    }
}

fn step3_modes(s: &mut Session) -> (Outcome, String) {
    let packets = match s.request(31, 1, Op::Start, &[], true, Duration::from_millis(2800)) {
        Ok(p) => p,
        Err(e) => return (Outcome::Fail, format!("GetAll failed: {e}")),
    };
    let complete = packets.iter().any(|p| p.is(31, 1) && p.op == Op::Result);
    let records: Vec<&Packet> = packets.iter().filter(|p| p.is(31, 6) && p.op == Op::Status).collect();
    let mut walk = None;
    for r in &records {
        let b = &r.payload;
        if b.len() < 47 {
            println!("    slot ? short record [{}]", bmap::hex(b));
            continue;
        }
        let stored = bmap::string(&b[6..38]);
        let in_use = b[5] != 0;
        let name = if !stored.is_empty() { stored } else {
            bmap::prompt_name(b[2]).map(str::to_string).unwrap_or_else(|| "(unnamed)".into())
        };
        if b[2] == 0x15 && in_use {
            walk = Some(b[0]);
        }
        println!(
            "    slot {}: {:<10} editable={} in_use={} cnc={} wind_block={}",
            b[0], name, b[3] != 0, in_use, b[42], b[46] != 0
        );
    }
    if !complete || records.len() < 2 {
        return (Outcome::Fail, format!("complete={complete}, records={}", records.len()));
    }

    let current = match s.get(31, 3, &[]) {
        Ok(p) => p.payload.first().copied().unwrap_or(0),
        Err(e) => return (Outcome::Fail, format!("read current mode: {e}")),
    };
    let target = walk.filter(|&w| w != current).unwrap_or(if current == 1 { 0 } else { 1 });
    println!("    switching mode {current} → {target} with the voice prompt…");
    if let Err(e) = s.single(31, 3, Op::Start, &[target, 1], Duration::from_secs(3)) {
        return (Outcome::Fail, format!("switch failed: {e}"));
    }
    std::thread::sleep(Duration::from_millis(400));
    let now = s.get(31, 3, &[]).ok().and_then(|p| p.payload.first().copied());
    let heard = ask("    Did the headset announce the new mode?");
    println!("    restoring mode {current}…");
    let _ = s.single(31, 3, Op::Start, &[current, 1], Duration::from_secs(3));
    if now == Some(target) && heard {
        (Outcome::Pass, format!("{} records, switched {current}→{target}, prompt heard", records.len()))
    } else {
        (Outcome::Fail, format!("read back {now:?} (wanted {target}), prompt heard={heard}"))
    }
}

fn step4_peers(s: &mut Session) -> (Outcome, String) {
    match s.peers() {
        Ok(peers) => {
            let active = s.active_source();
            print_peers(&peers, active.as_deref());
            let locals = peers.iter().filter(|p| p.local).count();
            let summary = peers
                .iter()
                .map(|p| format!("{}:{}{}", p.name, if p.connected { "on" } else { "off" }, if p.local { "/local" } else { "" }))
                .collect::<Vec<_>>()
                .join(", ");
            let ok = !peers.is_empty() && locals == 1;
            (if ok { Outcome::Pass } else { Outcome::Fail },
             format!("{summary}; source={}; local entries={locals}", active.unwrap_or_else(|| "-".into())))
        }
        Err(e) => (Outcome::Fail, format!("4.4 failed: {e}")),
    }
}

fn step5_disconnect_reconnect(s: &mut Session) -> (Outcome, String) {
    let peers = match s.peers() {
        Ok(p) => p,
        Err(e) => return (Outcome::Fail, format!("4.4 failed: {e}")),
    };
    let candidates: Vec<&Peer> = peers.iter().filter(|p| p.connected && !p.local).collect();
    if candidates.is_empty() {
        return (Outcome::Skip, "no other device connected; connect your phone to the headset and rerun".into());
    }
    let labels: Vec<String> = candidates.iter().map(|p| format!("{} {}", p.name, p.mac)).collect();
    let Some(i) = choose("    Disconnect and reconnect which device?", &labels) else {
        return (Outcome::Skip, "skipped by user".into());
    };
    let (name, mac) = (candidates[i].name.clone(), candidates[i].mac.clone());
    let dropped = match s.disconnect(&mac) {
        Ok(d) => d,
        Err(e) => return (Outcome::Fail, format!("disconnect {name}: {e}")),
    };
    println!("    {name} disconnected: {dropped}");
    let back = match s.connect(&mac) {
        Ok(c) => c,
        Err(e) => return (Outcome::Fail, format!("reconnect {name}: {e}")),
    };
    println!("    {name} reconnected: {back}");
    (if dropped && back { Outcome::Pass } else { Outcome::Fail },
     format!("{name}: disconnected={dropped}, reconnected={back}"))
}

fn step6_auto_drop(s: &mut Session) -> (Outcome, String) {
    let peers = match s.peers() {
        Ok(p) => p,
        Err(e) => return (Outcome::Fail, format!("4.4 failed: {e}")),
    };
    let connected: Vec<&Peer> = peers.iter().filter(|p| p.connected).collect();
    let targets: Vec<&Peer> = peers.iter().filter(|p| !p.connected && !p.local).collect();
    if connected.len() < 2 || targets.is_empty() {
        return (Outcome::Skip, format!(
            "needs both slots full and a third paired device (connected={}, available={})",
            connected.len(), targets.len()));
    }
    print_peers(&peers, None);
    let labels: Vec<String> = targets.iter().map(|p| format!("{} {}", p.name, p.mac)).collect();
    let Some(i) = choose("    Switch to which device?", &labels) else {
        return (Outcome::Skip, "skipped by user".into());
    };
    let target = targets[i].mac.clone();
    let Some(victim) = connected.iter().find(|p| !p.local).map(|p| (p.name.clone(), p.mac.clone())) else {
        return (Outcome::Skip, "no non-PC device to drop".into());
    };
    println!("    dropping {} (the non-PC device), then connecting the target…", victim.0);
    let dropped = s.disconnect(&victim.1).unwrap_or(false);
    let joined = s.connect(&target).unwrap_or(false);
    let pc_still = s.peers().map(|ps| ps.iter().any(|p| p.local && p.connected)).unwrap_or(false);
    (if dropped && joined && pc_still { Outcome::Pass } else { Outcome::Fail },
     format!("dropped {}={dropped}, target connected={joined}, PC still connected={pc_still}", victim.0))
}

fn step7_connect_audio(s: &mut Session) -> (Outcome, String) {
    let peers = match s.peers() {
        Ok(p) => p,
        Err(e) => return (Outcome::Fail, format!("4.4 failed: {e}")),
    };
    let Some(local) = peers.iter().find(|p| p.local) else {
        return (Outcome::Fail, "this PC isn't in the headset's device list".into());
    };
    let mac = local.mac.clone();
    if !ask("    This disconnects the PC's audio from the headset, then asks the headset to reconnect it. Continue?") {
        return (Outcome::Skip, "skipped by user".into());
    }
    let mut notes = Vec::new();
    if local.connected {
        let dropped = s.disconnect(&mac).unwrap_or(false);
        notes.push(format!("PC audio dropped={dropped}"));
    }
    let link_alive = s.get(2, 2, &[]).is_ok();
    notes.push(format!("control link survived={link_alive}"));
    if !link_alive {
        match s.reopen() {
            Ok(()) => notes.push("reopened link".into()),
            Err(e) => return (Outcome::Fail, format!("{}; reopen failed: {e}", notes.join(", "))),
        }
    }
    let reconnected = s.connect(&mac);
    notes.push(format!("CONNECT self → {reconnected:?}"));
    let audio = ask("    Does Windows show the headset as the audio output again, and does audio play?");
    notes.push(format!("Windows audio back={audio}"));
    (if audio { Outcome::Pass } else { Outcome::Fail }, notes.join(", "))
}

fn step8_stress(s: &mut Session, seconds: u64) -> (Outcome, String) {
    println!("    running mixed requests for {seconds}s…");
    let end = Instant::now() + Duration::from_secs(seconds);
    let (mut rounds, mut wrong, mut errors, mut incomplete) = (0, 0, 0, 0);
    while Instant::now() < end {
        for (fb, f) in [(2u8, 2u8), (31, 3), (4, 4)] {
            match s.get(fb, f, &[]) {
                Ok(_) => {}
                Err(e) if e.contains("reply came from") => wrong += 1,
                Err(_) => errors += 1,
            }
        }
        match s.request(31, 1, Op::Start, &[], true, Duration::from_millis(2800)) {
            Ok(p) if p.iter().any(|x| x.is(31, 1) && x.op == Op::Result) => {}
            Ok(_) => incomplete += 1,
            Err(_) => errors += 1,
        }
        rounds += 1;
    }
    let ok = wrong == 0 && errors == 0 && incomplete == 0;
    (if ok { Outcome::Pass } else { Outcome::Fail },
     format!("{rounds} rounds; wrong-address replies={wrong}, errors={errors}, incomplete GetAll={incomplete}"))
}

// ── Runner ───────────────────────────────────────────────────────────────────

struct Args {
    address: Option<String>,
    steps: Vec<u8>,
    stress_seconds: u64,
}

fn parse_args() -> Args {
    let mut args = Args { address: None, steps: (1..=8).collect(), stress_seconds: 60 };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--address" => args.address = it.next(),
            "--steps" => {
                args.steps = it.next().unwrap_or_default().split(',').filter_map(|s| s.trim().parse().ok()).collect()
            }
            "--stress-seconds" => args.stress_seconds = it.next().and_then(|s| s.parse().ok()).unwrap_or(60),
            other => eprintln!("ignoring unknown argument {other}"),
        }
    }
    args
}

pub fn run() {
    START.with(|_| {});
    let args = parse_args();
    println!("QC Buddy M0 spike\n");

    let devices = paired::list();
    println!("Paired Bluetooth devices on this PC:");
    for d in &devices {
        println!("  {} {:<30} {}{}", if d.connected { "●" } else { "○" }, d.name, d.mac(),
                 if d.looks_like_bose() { "  (Bose?)" } else { "" });
    }
    let address = match &args.address {
        Some(mac) => bmap::mac_bytes(mac).map(|b| b.iter().fold(0u64, |acc, x| (acc << 8) | *x as u64)),
        None => devices
            .iter()
            .filter(|d| d.looks_like_bose())
            .max_by_key(|d| d.connected)
            .map(|d| d.address),
    };
    let Some(address) = address else {
        eprintln!("\nNo Bose headset found by name. Rerun with --address XX:XX:XX:XX:XX:XX (from the list above).");
        std::process::exit(2);
    };

    let traffic = File::create("spike-traffic.log").expect("create spike-traffic.log");
    let mut results: Vec<StepResult> = Vec::new();

    println!("\n[1] Open RFCOMM (channels 8, 2, 9) while audio plays");
    let link = match Rfcomm::open(address, &CHANNELS) {
        Ok(l) => l,
        Err(e) => {
            results.push(StepResult { number: 1, title: "Open RFCOMM while audio plays", outcome: Outcome::Fail, note: e });
            write_report(&results);
            return;
        }
    };
    let mut s = Session { address, traffic, link };
    s.log(&format!("connected on channel {}", s.link.channel));
    let audio_ok = ask("    Connected. Is audio still playing normally?");
    results.push(StepResult {
        number: 1,
        title: "Open RFCOMM while audio plays",
        outcome: if audio_ok { Outcome::Pass } else { Outcome::Fail },
        note: format!("channel {}, audio still playing={audio_ok}", s.link.channel),
    });

    let steps: [(u8, &'static str); 7] = [
        (2, "Read battery, firmware, name"),
        (3, "Mode list (GetAll) and switch with prompt"),
        (4, "Paired-device list (4.4/4.5)"),
        (5, "Disconnect and reconnect a device (4.2/4.1)"),
        (6, "Auto-drop when both slots are full"),
        (7, "Connect audio for this PC (CONNECT self)"),
        (8, "Serialized requests under load"),
    ];
    for (number, title) in steps {
        if !args.steps.contains(&number) {
            continue;
        }
        println!("\n[{number}] {title}");
        s.log(&format!("── step {number}: {title}"));
        let (outcome, note) = match number {
            2 => step2_reads(&mut s),
            3 => step3_modes(&mut s),
            4 => step4_peers(&mut s),
            5 => step5_disconnect_reconnect(&mut s),
            6 => step6_auto_drop(&mut s),
            7 => step7_connect_audio(&mut s),
            _ => step8_stress(&mut s, args.stress_seconds),
        };
        println!("    → {} {note}", label(outcome));
        results.push(StepResult { number, title, outcome, note });
    }
    write_report(&results);
}

fn label(o: Outcome) -> &'static str {
    match o {
        Outcome::Pass => "PASS",
        Outcome::Fail => "FAIL",
        Outcome::Skip => "SKIP",
    }
}

fn write_report(results: &[StepResult]) {
    let mut out = String::from("QC Buddy M0 spike report\n\n");
    for r in results {
        out.push_str(&format!("[{}] {:<4} {} — {}\n", r.number, label(r.outcome), r.title, r.note));
    }
    let passed = results.iter().filter(|r| r.outcome == Outcome::Pass).count();
    out.push_str(&format!("\n{passed}/{} passed. Traffic: spike-traffic.log\n", results.len()));
    print!("\n{out}");
    if let Err(e) = std::fs::write("spike-report.txt", &out) {
        eprintln!("couldn't write spike-report.txt: {e}");
    }
}
