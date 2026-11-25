//! BMAP packet framing and byte helpers (port of Protocol.swift).
//!
//! One packet: `[fblock, function, flags, length, payload...]`; the operator
//! is the low nibble of `flags`, the port bits 4-5.

use std::fmt;

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
    pub fn from_nibble(n: u8) -> Op {
        match n & 0x0F {
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

    /// Name used in log lines, matching BoseBuddy ("get", "setGet", "start").
    pub fn name(self) -> &'static str {
        match self {
            Op::Set => "set",
            Op::Get => "get",
            Op::SetGet => "setGet",
            Op::Status => "status",
            Op::Error => "error",
            Op::Start => "start",
            Op::Result => "result",
            Op::Processing => "processing",
        }
    }
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub fblock: u8,
    pub function: u8,
    pub port: u8,
    pub op: Op,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new(fblock: u8, function: u8, op: Op, payload: &[u8]) -> Packet {
        Packet { fblock, function, port: 0, op, payload: payload.to_vec() }
    }

    pub fn is(&self, fblock: u8, function: u8) -> bool {
        self.fblock == fblock && self.function == function
    }

    pub fn encode(&self) -> Vec<u8> {
        let body = &self.payload[..self.payload.len().min(255)];
        let flags = ((self.port & 0x03) << 4) | (self.op as u8 & 0x0F);
        let mut out = vec![self.fblock, self.function, flags, body.len() as u8];
        out.extend_from_slice(body);
        out
    }
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
        let flags = data[i + 2];
        packets.push(Packet {
            fblock: data[i],
            function: data[i + 1],
            port: (flags >> 4) & 0x03,
            op: Op::from_nibble(flags),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BmapError {
    NotConnected,
    NoResponse,
    Timeout,
    Transport(String),
    Device { code: u8, fblock: u8, function: u8 },
}

impl fmt::Display for BmapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BmapError::NotConnected => f.write_str("Not connected to the headphones."),
            BmapError::NoResponse => f.write_str("The headphones didn't return any data."),
            BmapError::Timeout => f.write_str("The headphones didn't respond in time."),
            BmapError::Transport(message) => f.write_str(message),
            BmapError::Device { code, fblock, function } => {
                write!(f, "Headphones returned {} for [{fblock}.{function}].", error_name(*code))
            }
        }
    }
}

impl std::error::Error for BmapError {}

/// Name of a BMAP error code.
pub fn error_name(code: u8) -> String {
    match code {
        0 => "unknown error",
        1 => "bad length",
        2 => "checksum error",
        3 => "unsupported block",
        4 => "unsupported function",
        5 => "operator not supported",
        6 => "invalid data",
        7 => "data unavailable",
        8 => "temporary failure",
        9 => "timeout",
        10 => "invalid state",
        11 => "device not found",
        12 => "device busy",
        15 => "invalid transition",
        20 => "insecure transport",
        _ => return format!("error {code}"),
    }
    .to_string()
}

/// "78:2B:64:5B:71:EE" or "78-2b-..." → bytes. Unparseable parts are skipped.
pub fn mac_bytes(mac: &str) -> Vec<u8> {
    mac.split([':', '-']).filter_map(|p| u8::from_str_radix(p, 16).ok()).collect()
}

/// Canonical display form, e.g. 78:2B:64:5B:71:EE.
pub fn format_mac(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

pub fn canonical_mac(raw: &str) -> String {
    let bytes = mac_bytes(raw);
    if bytes.len() == 6 {
        format_mac(&bytes)
    } else {
        raw.to_uppercase()
    }
}

/// A Bluetooth address as Winsock's u64 (0x782B645B71EE).
pub fn mac_to_u64(mac: &str) -> Option<u64> {
    let bytes = mac_bytes(mac);
    (bytes.len() == 6).then(|| bytes.iter().fold(0u64, |acc, b| (acc << 8) | *b as u64))
}

pub fn u64_to_mac(address: u64) -> String {
    format_mac(&address.to_be_bytes()[2..8])
}

/// Printable UTF-8 up to the first NUL, without leading/trailing control bytes.
pub fn string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let mut slice = &bytes[..end];
    while let Some((&first, rest)) = slice.split_first() {
        if first >= 0x20 {
            break;
        }
        slice = rest;
    }
    while let Some((&last, rest)) = slice.split_last() {
        if last >= 0x20 {
            break;
        }
        slice = rest;
    }
    String::from_utf8_lossy(slice).into_owned()
}

/// Product name from a 1.2 STATUS payload. The QC45 (fw 4.0.4) sends one
/// control byte (0x00) before the name, which `string` alone would stop at.
pub fn product_name(payload: &[u8]) -> String {
    match payload.split_first() {
        Some((&first, rest)) if first < 0x20 => string(rest),
        _ => string(payload),
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

/// Bytes from "31 2e 31" (test fixtures and logs).
pub fn from_hex(text: &str) -> Vec<u8> {
    text.split_whitespace().filter_map(|p| u8::from_str_radix(p, 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let packet = Packet::new(31, 3, Op::Start, &[2, 1]);
        let bytes = packet.encode();
        assert_eq!(bytes, vec![31, 3, 5, 2, 2, 1]);
        assert_eq!(parse_all(&bytes), vec![packet]);
    }

    #[test]
    fn port_bits() {
        let mut packet = Packet::new(1, 7, Op::SetGet, &[]);
        packet.port = 2;
        assert_eq!(packet.encode()[2], 0x22);
        assert_eq!(parse_all(&packet.encode())[0].port, 2);
    }

    #[test]
    fn completeness_and_partial_packets() {
        let mut two = Packet::new(2, 2, Op::Status, &[80]).encode();
        two.extend(Packet::new(31, 1, Op::Result, &[]).encode());
        assert!(is_complete(&two));
        assert!(!is_complete(&two[..two.len() - 1]));
        assert!(!is_complete(&[]));
        assert_eq!(parse_all(&two[..two.len() - 1]).len(), 1);
    }

    #[test]
    fn macs() {
        assert_eq!(mac_bytes("78:2B:64:5B:71:EE"), vec![0x78, 0x2B, 0x64, 0x5B, 0x71, 0xEE]);
        assert_eq!(canonical_mac("78-2b-64-5b-71-ee"), "78:2B:64:5B:71:EE");
        assert_eq!(canonical_mac("nope"), "NOPE");
        assert_eq!(mac_to_u64("78:2B:64:5B:71:EE"), Some(0x782B645B71EE));
        assert_eq!(u64_to_mac(0x782B645B71EE), "78:2B:64:5B:71:EE");
        assert_eq!(mac_to_u64("78:2B"), None);
    }

    #[test]
    fn strings() {
        assert_eq!(string(b"Quiet\0\0\0"), "Quiet");
        assert_eq!(string(b"\x01\x02Walk\x03"), "Walk");
        assert_eq!(string(b"\0Studio Cans"), "");
    }

    #[test]
    fn product_name_skips_leading_control_byte() {
        // 1.2 STATUS shape captured from a QC45 on fw 4.0.4 (name replaced).
        let payload = from_hex("00 53 74 75 64 69 6f 20 43 61 6e 73");
        assert_eq!(product_name(&payload), "Studio Cans");
        assert_eq!(product_name(b"Bose QC45"), "Bose QC45");
        assert_eq!(product_name(&[]), "");
    }

    #[test]
    fn errors_read_like_bosebuddy() {
        let e = BmapError::Device { code: 4, fblock: 2, function: 3 };
        assert_eq!(e.to_string(), "Headphones returned unsupported function for [2.3].");
        assert_eq!(error_name(99), "error 99");
    }
}
