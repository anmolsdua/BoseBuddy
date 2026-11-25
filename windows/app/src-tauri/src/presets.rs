//! The flyout's EQ preset commands. Every call re-reads the file, so presets
//! saved by the CLI (or another QC Buddy) show up, and returns the full list
//! for the flyout to render.

use std::path::Path;

use qcbuddy_core::eq_presets::{EqPreset, EqPresetStore};

pub fn list(path: &Path) -> Vec<EqPreset> {
    EqPresetStore::open(path).all()
}

pub fn save(path: &Path, name: &str, values: &[i32]) -> Result<Vec<EqPreset>, String> {
    let mut store = EqPresetStore::open(path);
    store.try_save(name, values)?;
    Ok(store.all())
}

pub fn rename(path: &Path, id: &str, name: &str) -> Result<Vec<EqPreset>, String> {
    let mut store = EqPresetStore::open(path);
    store.try_rename(id, name)?;
    Ok(store.all())
}

pub fn delete(path: &Path, id: &str) -> Vec<EqPreset> {
    let mut store = EqPresetStore::open(path);
    store.delete(id);
    store.all()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("qcbuddy-app-presets-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("eq-presets.json")
    }

    #[test]
    fn save_rename_delete_round_trip_through_the_file() {
        let path = temp_file("round-trip");
        assert_eq!(list(&path).len(), 6, "built-ins when there's no file yet");
        let after_save = save(&path, "Late night", &[-3, 1, -2]).unwrap();
        let mine = after_save.last().unwrap().clone();
        assert_eq!((mine.name.as_str(), mine.values.as_slice(), mine.is_built_in), ("Late night", &[-3, 1, -2][..], false));
        // A new call (as the next command would) reads it back from disk.
        assert_eq!(list(&path).last().unwrap().id, mine.id);
        let after_rename = rename(&path, &mine.id, "Bedtime").unwrap();
        assert_eq!(after_rename.last().unwrap().name, "Bedtime");
        assert_eq!(delete(&path, &mine.id).len(), 6);
        assert_eq!(list(&path).len(), 6);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn errors_come_back_as_messages_for_the_flyout() {
        let path = temp_file("errors");
        assert_eq!(save(&path, "warm", &[1, 1, 1]).unwrap_err(), "\"warm\" is already a preset.");
        assert_eq!(rename(&path, "builtin.flat", "Mine").unwrap_err(), "Built-in presets can't be renamed.");
        assert!(!path.exists(), "nothing written on failure");
    }

    #[test]
    fn presets_saved_by_the_cli_show_up() {
        let path = temp_file("cli");
        // What `qcbuddy preset-save` does: its own store on the same file.
        EqPresetStore::open(&path).save("From CLI", &[2, 2, 2]);
        assert!(list(&path).iter().any(|p| p.name == "From CLI"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
