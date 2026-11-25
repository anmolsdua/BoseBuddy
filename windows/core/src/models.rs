//! Plain data shared by the controller, the CLI and the UI (port of Models.swift).

use serde::Serialize;

/// A Bose headset paired with this PC.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Headphones {
    pub address: String,
    pub name: String,
    pub connected: bool,
}

/// A device in the headset's own paired-device list (4.4 / 4.5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub mac: String,
    pub name: String,
    pub connected: bool,
    /// This PC, as the headset sees it.
    pub is_local: bool,
    pub is_bose_product: bool,
    /// The device whose audio the headset is playing.
    pub active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PeerKind {
    Computer,
    Phone,
    Tablet,
    Headphones,
}

impl Peer {
    pub fn unknown(mac: &str) -> Peer {
        Peer {
            mac: mac.into(),
            name: mac.into(),
            connected: false,
            is_local: false,
            is_bose_product: false,
            active: false,
        }
    }

    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.mac
        } else {
            &self.name
        }
    }

    /// Icon choice for the Devices card.
    pub fn kind(&self) -> PeerKind {
        if self.is_local {
            return PeerKind::Computer;
        }
        let lower = self.name.to_lowercase();
        if lower.contains("ipad") || lower.contains("pad") {
            PeerKind::Tablet
        } else if lower.contains("iphone") || lower.contains("phone") || lower.contains("pixel") {
            PeerKind::Phone
        } else if self.is_bose_product {
            PeerKind::Headphones
        } else {
            PeerKind::Computer
        }
    }

    pub fn status_text(&self) -> &'static str {
        match (self.is_local, self.connected, self.active) {
            (true, _, true) => "This PC · Audio",
            (true, _, false) => "This PC",
            (false, true, true) => "Connected · Audio",
            (false, true, false) => "Connected",
            (false, false, _) => "Not connected",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EqBand {
    pub band_id: u8,
    pub name: String,
    pub minimum: i32,
    pub maximum: i32,
    pub value: i32,
}

impl EqBand {
    pub fn clamp(&self, value: i32) -> i32 {
        value.clamp(self.minimum, self.maximum)
    }
}

/// "Both slots are full; which device should be dropped?" (ask-which option).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerSwitch {
    pub target: Peer,
    pub candidates: Vec<Peer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    None,
    Bluetooth,
    Auxiliary,
}

impl SourceKind {
    pub fn from_byte(b: u8) -> SourceKind {
        match b {
            1 => SourceKind::Bluetooth,
            2 => SourceKind::Auxiliary,
            _ => SourceKind::None,
        }
    }
}
