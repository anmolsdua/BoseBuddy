//! Plain-English log text for BMAP requests, with the raw codes kept in
//! brackets (port of RequestDescription.swift), e.g.
//! "Headphones don't support reading charging state [2.3 get, error 4 unsupported function]".

use crate::protocol::{error_name, BmapError, Op};

/// "reading charging state", "changing EQ", "requesting mode list".
pub fn action(fblock: u8, function: u8, op: Op) -> String {
    let feature = feature(fblock, function)
        .map(str::to_string)
        .unwrap_or_else(|| format!("function {fblock}.{function}"));
    match op {
        Op::Get => format!("reading {feature}"),
        Op::Set | Op::SetGet => format!("changing {feature}"),
        _ => format!("requesting {feature}"),
    }
}

/// The headphones answered with an error packet.
pub fn device_error(fblock: u8, function: u8, op: Op, code: u8) -> String {
    let action = action(fblock, function, op);
    let sentence = match code {
        3 | 4 => format!("Headphones don't support {action}"),
        5 => format!("Headphones don't allow {action}"),
        6 => format!("Headphones rejected the value when {action}"),
        7 => format!("Headphones have no data for {action}"),
        8 | 12 => format!("Headphones were busy and couldn't finish {action}"),
        1 => format!("Headphones rejected the message length when {action}"),
        _ => format!("Headphones returned an error when {action}"),
    };
    format!("{sentence} [{fblock}.{function} {op}, error {code} {}]", error_name(code))
}

/// The request itself failed: no reply, not connected, or a transport error.
pub fn failure(fblock: u8, function: u8, op: Op, error: &BmapError, seconds: f64) -> String {
    let action = action(fblock, function, op);
    let codes = format!(" [{fblock}.{function} {op}]");
    match error {
        BmapError::Timeout => {
            format!("No reply from the headphones when {action} after {seconds:.1}s{codes}")
        }
        BmapError::NotConnected => format!("Not connected when {action}{codes}"),
        other => format!("Failed {action}: {other}{codes}"),
    }
}

fn feature(fblock: u8, function: u8) -> Option<&'static str> {
    Some(match (fblock, function) {
        (0, 5) => "firmware version",
        (1, 2) => "headphone name",
        (1, 3) => "voice prompts",
        (1, 5) => "noise cancelling level",
        (1, 7) => "EQ",
        (1, 9) => "button settings",
        (1, 10) => "multipoint",
        (1, 11) => "sidetone",
        (2, 2) => "battery level",
        (2, 3) => "charging state",
        (4, 1) => "device connection",
        (4, 2) => "device disconnection",
        (4, 4) => "paired device list",
        (4, 5) => "device info",
        (4, 8) => "pairing mode",
        (5, 1) => "audio source",
        (7, 4) => "power",
        (31, 1) => "mode list",
        (31, 3) => "current mode",
        (31, 6) => "mode settings",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Same cases as Tests/RequestDescriptionTests.swift.
    #[test]
    fn descriptions() {
        // The QC45 rejects the charging-state read.
        assert_eq!(
            device_error(2, 3, Op::Get, 4),
            "Headphones don't support reading charging state [2.3 get, error 4 unsupported function]"
        );
        assert_eq!(
            device_error(1, 7, Op::SetGet, 6),
            "Headphones rejected the value when changing EQ [1.7 setGet, error 6 invalid data]"
        );
        assert_eq!(
            failure(31, 1, Op::Start, &BmapError::Timeout, 2.84),
            "No reply from the headphones when requesting mode list after 2.8s [31.1 start]"
        );
        // Unknown requests still read sensibly.
        assert_eq!(action(9, 9, Op::Get), "reading function 9.9");
    }
}
