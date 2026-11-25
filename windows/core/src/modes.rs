//! Noise-control modes as stored on the headset (AudioModes, fblock 31).
//! Port of NoiseModes.swift.
//!
//! ModeConfig [31.6] STATUS layout (47 bytes, QC45 / QC Headphones):
//!   [0] index, [1-2] voice prompt ID, [3] editable, [4] configured,
//!   [5] slot in use, [6-37] name, [41] capability bits, [42] CNC level,
//!   [43] auto CNC, [44] spatial, [46] wind block.
//! SETGET takes 39 bytes: index, prompt (2), name (32), CNC, auto CNC,
//! spatial, wind block. Layout from bosectl (github.com/aaronsb/bosectl),
//! confirmed on a QC45 running firmware 4.0.4.

use serde::Serialize;

use crate::protocol::string;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoseMode {
    pub index: u8,
    pub name: String,
    /// 0 is full noise cancelling, 10 is fully aware.
    pub raw_cnc: u8,
    /// Wind Block adjusts noise cancelling automatically; while it is on the
    /// headset stores CNC 0 regardless of what is written.
    pub wind_block: bool,
    /// User modes (slots 2-3 on a QC45) can be rewritten; Quiet/Aware can't.
    pub editable: bool,
    pub prompt: [u8; 2],
    /// Stored name bytes; empty for Bose's ready-made modes, named by prompt.
    pub stored_name: String,
    pub auto_cnc: bool,
    pub spatial: u8,
}

pub const CUSTOM_NAME: &str = "Custom";

impl BoseMode {
    pub fn new(index: u8, name: &str, raw_cnc: u8) -> BoseMode {
        BoseMode {
            index,
            name: name.into(),
            raw_cnc,
            wind_block: false,
            editable: false,
            prompt: [0, 0],
            stored_name: String::new(),
            auto_cnc: false,
            spatial: 0,
        }
    }

    /// Used when the headset doesn't report its modes.
    pub fn fallback() -> Vec<BoseMode> {
        vec![BoseMode::new(0, "Quiet", 0), BoseMode::new(1, "Aware", 10)]
    }

    /// Noise cancelling strength shown to the user: 10 is full ANC.
    pub fn anc_level(&self) -> u8 {
        10 - self.raw_cnc.min(10)
    }

    /// Text for the Noise control header.
    pub fn anc_label(&self) -> String {
        if self.wind_block {
            "ANC Auto · Wind Block".into()
        } else {
            format!("ANC {}/10", self.anc_level())
        }
    }

    /// Parses one mode record (31.6). `None` for unused slots.
    pub fn from_record(b: &[u8]) -> Option<BoseMode> {
        if b.len() < 38 || b[0] == 0xFF {
            return None;
        }
        let index = b[0];
        let stored_name = string(&b[6..38]);
        let in_use = b[5] != 0;
        if stored_name.is_empty() && index > 1 && !in_use {
            return None;
        }
        let full = b.len() >= 47;
        let short_layout = full && b.len() < 48;
        let cnc_offset = if b.len() >= 48 { 38 } else { 42 };
        let prompt_name = prompt_name(b[2]);
        // Our Custom mode has no prompt. When the Bose app reuses its slot it
        // writes a template prompt but can leave our name bytes behind, so a
        // prompt wins over a leftover "Custom".
        let taken_over = stored_name == CUSTOM_NAME && prompt_name.is_some();
        let name = if stored_name.is_empty() || taken_over {
            prompt_name.map(str::to_string).unwrap_or_else(|| default_name(index))
        } else {
            stored_name.clone()
        };
        Some(BoseMode {
            index,
            name,
            raw_cnc: b.get(cnc_offset).copied().unwrap_or(0),
            wind_block: short_layout && b[46] != 0,
            editable: b[3] != 0,
            prompt: [b[1], b[2]],
            stored_name,
            auto_cnc: short_layout && b[43] != 0,
            spatial: if short_layout { b[44] } else { 0 },
        })
    }

    /// Whether a record describes an editable slot with no mode in it.
    pub fn is_free_slot(b: &[u8]) -> bool {
        b.len() >= 38 && b[0] != 0xFF && b[3] != 0 && b[5] == 0 && string(&b[6..38]).is_empty()
    }

    /// 39-byte ModeConfig SETGET payload that rewrites this mode, keeping its
    /// prompt, name, auto CNC and spatial settings.
    pub fn config_payload(&self, raw_cnc: u8, wind_block: bool) -> Vec<u8> {
        let mut name: Vec<u8> = self.stored_name.as_bytes().iter().take(31).copied().collect();
        name.resize(32, 0);
        let mut out = vec![self.index, self.prompt[0], self.prompt[1]];
        out.extend(name);
        out.extend([raw_cnc.min(10), self.auto_cnc as u8, self.spatial, wind_block as u8]);
        out
    }

    /// A new user mode named "Custom" for a free slot.
    pub fn custom(slot: u8, raw_cnc: u8) -> BoseMode {
        BoseMode {
            editable: true,
            stored_name: CUSTOM_NAME.into(),
            ..BoseMode::new(slot, CUSTOM_NAME, raw_cnc)
        }
    }
}

/// Bose's mode voice prompts, keyed by the prompt ID's low byte. The headset
/// stores no name for ready-made modes such as Walk (0x15).
pub fn prompt_name(id: u8) -> Option<&'static str> {
    const NAMES: [&str; 36] = [
        "Quiet", "Aware", "Transparent", "Transparency", "Masking", "Comfort", "Commute",
        "Outdoor", "Workout", "Home", "Work", "Music", "Focus", "Relax", "Flight", "Airport",
        "Driving", "Training", "Gym", "Run", "Walk", "Hike", "Talk", "Call", "Whisper",
        "Hearing", "Learn", "Podcast", "Audiobook", "Calm", "Sleep", "Meditate", "Yoga",
        "Immersion", "Stereo", "Cinema",
    ];
    (id as usize).checked_sub(1).and_then(|i| NAMES.get(i)).copied()
}

fn default_name(index: u8) -> String {
    match index {
        0 => "Quiet".into(),
        1 => "Aware".into(),
        _ => format!("Mode {}", index as u32 + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::from_hex;

    // Mode records captured from a QC45 (firmware 4.0.4); same as
    // Tests/NoiseModeTests.swift.
    const QUIET: &str = "00 00 01 00 00 01 51 75 69 65 74 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 00 00 00 00 00";
    const AWARE: &str = "01 00 02 00 00 01 41 77 61 72 65 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 0a 00 00 00 00";
    const WALK: &str = "02 00 15 01 01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 00 00 00 00 01";
    const OUTDOOR: &str = "03 00 08 01 01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 03 00 00 00 00";
    const EMPTY: &str = "03 00 00 01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 00 00 00 00 00";
    const OURS: &str = "03 00 00 01 01 01 43 75 73 74 6f 6d 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 00 00 00 00 00";
    const TAKEN_OVER: &str = "03 00 07 01 01 01 43 75 73 74 6f 6d 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 05 00 00 00 00";

    fn mode(hex: &str) -> BoseMode {
        BoseMode::from_record(&from_hex(hex)).expect("mode")
    }

    #[test]
    fn quiet_and_aware() {
        let quiet = mode(QUIET);
        assert_eq!((quiet.index, quiet.name.as_str(), quiet.raw_cnc), (0, "Quiet", 0));
        assert_eq!(quiet.anc_label(), "ANC 10/10");
        assert!(!quiet.editable);
        let aware = mode(AWARE);
        assert_eq!((aware.index, aware.name.as_str(), aware.raw_cnc), (1, "Aware", 10));
        assert_eq!(aware.anc_label(), "ANC 0/10");
    }

    #[test]
    fn ready_made_modes_are_named_by_prompt() {
        // Unnamed ready-made mode: named from its template ID, Wind Block on.
        let walk = mode(WALK);
        assert_eq!(walk.name, "Walk");
        assert!(walk.wind_block);
        assert!(walk.editable);
        assert_eq!(walk.anc_label(), "ANC Auto · Wind Block");

        // Wind Block is byte 46, not the capability bits in byte 41, which
        // Outdoor shares with Walk.
        let outdoor = mode(OUTDOOR);
        assert_eq!(outdoor.name, "Outdoor");
        assert!(!outdoor.wind_block);
        assert_eq!(outdoor.anc_label(), "ANC 7/10");

        let mut unknown = from_hex(WALK);
        unknown[2] = 0x7E;
        assert_eq!(BoseMode::from_record(&unknown).unwrap().name, "Mode 3");
    }

    #[test]
    fn free_slots() {
        // Unused slots are skipped and reported as free; used ones aren't.
        assert_eq!(BoseMode::from_record(&from_hex(EMPTY)), None);
        assert!(BoseMode::is_free_slot(&from_hex(EMPTY)));
        assert!(!BoseMode::is_free_slot(&from_hex(OUTDOOR)));
        assert!(!BoseMode::is_free_slot(&from_hex(QUIET)));
    }

    #[test]
    fn custom_mode_naming() {
        // Our Custom mode keeps its name; once the Bose app reuses the slot
        // (prompt 0x07 Commute, old name bytes left behind) the prompt wins.
        assert_eq!(mode(OURS).name, "Custom");
        let taken = mode(TAKEN_OVER);
        assert_eq!(taken.name, "Commute");
        assert_eq!(taken.anc_level(), 5);
        let mut renamed = from_hex(OURS);
        renamed[6] = b'M'; // a name the user chose is kept
        assert_eq!(BoseMode::from_record(&renamed).unwrap().name, "Mustom");
    }

    #[test]
    fn config_payloads() {
        // Writes are 39 bytes: index, prompt, name, CNC, auto, spatial, wind.
        let write = mode(OUTDOOR).config_payload(5, false);
        assert_eq!(write.len(), 39);
        assert_eq!(&write[..3], &[3, 0x00, 0x08]);
        assert!(write[3..35].iter().all(|b| *b == 0));
        assert_eq!(&write[35..], &[5, 0, 0, 0]);
        let walk = mode(WALK);
        assert_eq!(&walk.config_payload(3, true)[35..], &[3, 0, 0, 1]);
        assert_eq!(walk.config_payload(42, false)[35], 10);

        let custom = BoseMode::custom(3, 4).config_payload(4, false);
        assert_eq!(&custom[..3], &[3, 0, 0]);
        assert_eq!(string(&custom[3..35]), "Custom");
        assert_eq!(&custom[35..], &[4, 0, 0, 0]);
    }

    #[test]
    fn rejects_short_and_ff_records() {
        let quiet = from_hex(QUIET);
        assert_eq!(BoseMode::from_record(&quiet[..20]), None);
        let mut none = quiet.clone();
        none[0] = 0xFF;
        assert_eq!(BoseMode::from_record(&none), None);
    }

    #[test]
    fn prompt_names() {
        assert_eq!(prompt_name(0x15), Some("Walk"));
        assert_eq!(prompt_name(0x24), Some("Cinema"));
        assert_eq!(prompt_name(0), None);
        assert_eq!(prompt_name(0x25), None);
    }
}
