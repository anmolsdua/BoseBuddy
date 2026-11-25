//! Named Bass/Mid/Treble settings (port of EQPresets.swift). Built-ins ship
//! with the app; custom ones are saved by the user in a JSON file.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqPreset {
    pub id: String,
    pub name: String,
    /// Band values ordered by EQ band ID (Bass, Mid, Treble).
    pub values: Vec<i32>,
    #[serde(default)]
    pub is_built_in: bool,
    /// Icon key for the chip; custom presets use "star".
    #[serde(default)]
    pub icon: Option<String>,
}

pub const MAX_NAME_LENGTH: usize = 13;

/// Trimmed and capped at 13 characters.
pub fn clean_name(raw: &str) -> String {
    raw.trim().chars().take(MAX_NAME_LENGTH).collect()
}

pub fn built_ins() -> Vec<EqPreset> {
    let preset = |id: &str, name: &str, values: [i32; 3], icon: &str| EqPreset {
        id: format!("builtin.{id}"),
        name: name.into(),
        values: values.to_vec(),
        is_built_in: true,
        icon: Some(icon.into()),
    };
    vec![
        preset("flat", "Flat", [0, 0, 0], "waveform"),
        preset("bass", "Bass", [6, 0, 0], "speaker"),
        preset("vocal", "Vocal", [-2, 4, 2], "mic"),
        preset("warm", "Warm", [4, 0, -3], "flame"),
        preset("bright", "Bright", [0, 0, 5], "sun"),
        preset("night", "Night", [-3, 2, -2], "moon"),
    ]
}

pub struct EqPresetStore {
    pub custom: Vec<EqPreset>,
    path: Option<PathBuf>,
}

impl EqPresetStore {
    /// Loads custom presets from `path` (missing or unreadable file = none).
    pub fn open(path: impl Into<PathBuf>) -> EqPresetStore {
        let path = path.into();
        let custom = std::fs::read(&path)
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default();
        EqPresetStore { custom, path: Some(path) }
    }

    /// A store that never touches disk.
    pub fn in_memory() -> EqPresetStore {
        EqPresetStore { custom: Vec::new(), path: None }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn all(&self) -> Vec<EqPreset> {
        let mut all = built_ins();
        all.extend(self.custom.iter().cloned());
        all
    }

    /// The first preset whose values equal the headset's current EQ.
    pub fn preset_matching(&self, values: &[i32]) -> Option<EqPreset> {
        if values.is_empty() {
            return None;
        }
        self.all().into_iter().find(|p| p.values == values)
    }

    /// Whether `name` is free to use, ignoring the preset being renamed.
    pub fn is_name_available(&self, name: &str, excluding: Option<&str>) -> bool {
        let clean = clean_name(name).to_lowercase();
        !clean.is_empty()
            && !self
                .all()
                .iter()
                .any(|p| Some(p.id.as_str()) != excluding && p.name.to_lowercase() == clean)
    }

    pub fn save(&mut self, name: &str, values: &[i32]) -> Option<EqPreset> {
        if !self.is_name_available(name, None) {
            return None;
        }
        let preset = EqPreset {
            id: new_id(),
            name: clean_name(name),
            values: values.to_vec(),
            is_built_in: false,
            icon: None,
        };
        self.custom.push(preset.clone());
        self.persist();
        Some(preset)
    }

    /// Like `save`, with the reason when the name can't be used (for the UI).
    pub fn try_save(&mut self, name: &str, values: &[i32]) -> Result<EqPreset, String> {
        self.check_name(name, None)?;
        self.save(name, values).ok_or_else(|| "Couldn't save the preset.".into())
    }

    /// Like `rename`, with the reason when it can't be done.
    pub fn try_rename(&mut self, id: &str, name: &str) -> Result<(), String> {
        if !self.custom.iter().any(|p| p.id == id) {
            return Err("Built-in presets can't be renamed.".into());
        }
        self.check_name(name, Some(id))?;
        self.rename(id, name);
        Ok(())
    }

    fn check_name(&self, name: &str, excluding: Option<&str>) -> Result<(), String> {
        let clean = clean_name(name);
        if clean.is_empty() {
            return Err("Give the preset a name.".into());
        }
        if !self.is_name_available(name, excluding) {
            return Err(format!("\"{clean}\" is already a preset."));
        }
        Ok(())
    }

    pub fn rename(&mut self, id: &str, name: &str) -> bool {
        if !self.is_name_available(name, Some(id)) {
            return false;
        }
        let Some(preset) = self.custom.iter_mut().find(|p| p.id == id) else {
            return false;
        };
        preset.name = clean_name(name);
        self.persist();
        true
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.custom.len();
        self.custom.retain(|p| p.id != id);
        let removed = self.custom.len() != before;
        if removed {
            self.persist();
        }
        removed
    }

    fn persist(&self) {
        let Some(path) = &self.path else { return };
        let Ok(data) = serde_json::to_vec_pretty(&self.custom) else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Write then rename, so a crash never leaves half a file.
        let temp = path.with_extension("json.tmp");
        if std::fs::write(&temp, data).is_ok() {
            let _ = std::fs::rename(&temp, path);
        }
    }
}

fn new_id() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("custom.{nanos:x}.{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Same cases as Tests/EQPresetTests.swift.
    #[test]
    fn presets() {
        let path = std::env::temp_dir().join(format!("qcbuddy-eq-test-{}.json", new_id()));
        let mut store = EqPresetStore::open(&path);

        // Names are trimmed and capped at 13 characters.
        assert_eq!(clean_name("  Late night  "), "Late night");
        assert_eq!(clean_name("Fourteen chars").chars().count(), MAX_NAME_LENGTH);

        // Built-ins are matched and highlighted by value.
        assert_eq!(store.preset_matching(&[0, 0, 0]).unwrap().name, "Flat");
        assert_eq!(store.preset_matching(&[8, 4, 2]), None);
        assert_eq!(store.preset_matching(&[]), None);

        // Saving, duplicate names, and blank names.
        let mine = store.save("Mine", &[8, 4, 2]).expect("saved");
        assert_eq!(store.preset_matching(&[8, 4, 2]).unwrap().id, mine.id);
        assert!(store.save("mine", &[1, 1, 1]).is_none());
        assert!(store.save("flat", &[1, 1, 1]).is_none());
        assert!(store.save("   ", &[1, 1, 1]).is_none());

        // Renaming keeps the cap, allows the preset's own name, and rejects others'.
        assert!(store.rename(&mine.id, "Commute mix for trains"));
        assert_eq!(store.custom[0].name, "Commute mix f");
        assert!(store.is_name_available("commute mix f", Some(&mine.id)));
        assert!(!store.rename(&mine.id, "Bass"));
        assert_eq!(store.custom[0].name, "Commute mix f");

        // Custom presets persist across launches; deleting removes them.
        let names: Vec<String> = EqPresetStore::open(&path).custom.into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Commute mix f"]);
        assert!(store.delete(&mine.id));
        assert!(EqPresetStore::open(&path).custom.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn errors_say_why() {
        let mut store = EqPresetStore::in_memory();
        assert_eq!(store.try_save("  ", &[1, 2, 3]).unwrap_err(), "Give the preset a name.");
        assert_eq!(store.try_save("bass", &[1, 2, 3]).unwrap_err(), "\"bass\" is already a preset.");
        let saved = store.try_save("Commute mix for trains", &[1, 2, 3]).unwrap();
        assert_eq!(saved.name, "Commute mix f");
        assert_eq!(store.try_rename("builtin.flat", "Mine").unwrap_err(), "Built-in presets can't be renamed.");
        assert_eq!(store.try_rename(&saved.id, "Warm").unwrap_err(), "\"Warm\" is already a preset.");
        assert_eq!(store.try_rename(&saved.id, "commute mix f"), Ok(()), "its own name, other case");
        assert_eq!(store.try_rename(&saved.id, "Trains"), Ok(()));
        assert_eq!(store.custom[0].name, "Trains");
    }

    #[test]
    fn unreadable_file_means_no_custom_presets() {
        let path = std::env::temp_dir().join(format!("qcbuddy-eq-bad-{}.json", new_id()));
        std::fs::write(&path, b"not json").unwrap();
        assert!(EqPresetStore::open(&path).custom.is_empty());
        let _ = std::fs::remove_file(&path);
    }
}
