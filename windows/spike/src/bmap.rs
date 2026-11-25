//! BMAP packet framing, mirroring BoseBuddy's `BmapPacket` (Protocol.swift).
//! One packet: [fblock, function, flags, length, payload...]; the operator is
//! the low nibble of `flags`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Set = 0,
    Get = 1,
    SetGet = 2,
    Status = 3,
    Error = 4,
    Start = 5,
    Result = 6,
    Processing = 7,
}

impl Op {
    fn from_nibble(n: u8) -> Op {
        match n {
            0 => Op::Set,
            1 => Op::Get,
            2 => Op::SetGet,
            4 => Op::Error,
            5 => Op::Start,
            6 => Op::Result,
            7 => Op::Processing,
            _ => Op::Status,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Packet {
    pub fblock: u8,
    pub function: u8,
    pub op: Op,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn is(&self, fblock: u8, function: u8) -> bool {
        self.fblock == fblock && self.function == function
    }
}

pub fn encode(fblock: u8, function: u8, op: Op, payload: &[u8]) -> Vec<u8> {
    let body = &payload[..payload.len().min(255)];
    let mut out = vec![fblock, function, op as u8 & 0x0F, body.len() as u8];
    out.extend_from_slice(body);
    out
}

/// Every complete packet in `data`; a partial trailing packet is ignored.
pub fn parse_all(data: &[u8]) -> Vec<Packet> {
    let mut packets = Vec::new();
    let mut i = 0;
    while i + 4 <= data.len() {
        let end = i + 4 + data[i + 3] as usize;
        if end > data.len() {
            break;
        }
        packets.push(Packet {
            fblock: data[i],
            function: data[i + 1],
            op: Op::from_nibble(data[i + 2] & 0x0F),
            payload: data[i + 4..end].to_vec(),
        });
        i = end;
    }
    packets
}

/// True when `data` is one or more whole packets with nothing left over.
pub fn is_complete(data: &[u8]) -> bool {
    let mut i = 0;
    let mut count = 0;
    while i + 4 <= data.len() {
        let end = i + 4 + data[i + 3] as usize;
        if end > data.len() {
            return false;
        }
        i = end;
        count += 1;
    }
    count > 0 && i == data.len()
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

/// Printable string up to the first NUL.
pub fn string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

pub fn format_mac(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

pub fn mac_bytes(mac: &str) -> Option<[u8; 6]> {
    let parts: Vec<u8> = mac
        .split([':', '-'])
        .filter_map(|p| u8::from_str_radix(p, 16).ok())
        .collect();
    parts.try_into().ok()
}

/// Names for Bose's mode voice prompts (low byte of the prompt ID), as in
/// BoseBuddy's NoiseModes.swift.
pub fn prompt_name(id: u8) -> Option<&'static str> {
    const NAMES: [&str; 36] = [
        "Quiet", "Aware", "Transparent", "Transparency", "Masking", "Comfort", "Commute",
        "Outdoor", "Workout", "Home", "Work", "Music", "Focus", "Relax", "Flight", "Airport",
        "Driving", "Training", "Gym", "Run", "Walk", "Hike", "Talk", "Call", "Whisper",
        "Hearing", "Learn", "Podcast", "Audiobook", "Calm", "Sleep", "Meditate", "Yoga",
        "Immersion", "Stereo", "Cinema",
    ];
    if id == 0 {
        None
    } else {
        NAMES.get(id as usize - 1).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let pkt = encode(31, 3, Op::Start, &[2, 1]);
        assert_eq!(pkt, vec![31, 3, 5, 2, 2, 1]);
        let parsed = parse_all(&pkt);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].op, Op::Start);
        assert_eq!(parsed[0].payload, vec![2, 1]);
    }

    #[test]
    fn completeness() {
        let mut two = encode(2, 2, Op::Status, &[80]);
        two.extend(encode(31, 1, Op::Result, &[]));
        assert!(is_complete(&two));
        assert!(!is_complete(&two[..two.len() - 1]));
        assert!(!is_complete(&[]));
    }

    #[test]
    fn names_and_macs() {
        assert_eq!(prompt_name(0x15), Some("Walk"));
        assert_eq!(prompt_name(0x08), Some("Outdoor"));
        assert_eq!(prompt_name(0), None);
        assert_eq!(mac_bytes("78:2B:64:5B:71:EE"), Some([0x78, 0x2B, 0x64, 0x5B, 0x71, 0xEE]));
        assert_eq!(format_mac(&[0x78, 0x2B, 0x64, 0x5B, 0x71, 0xEE]), "78:2B:64:5B:71:EE");
        assert_eq!(string(b"Quiet\0\0\0"), "Quiet");
    }
}
