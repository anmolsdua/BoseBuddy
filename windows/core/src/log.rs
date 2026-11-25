//! Append-only log, same lines as BoseBuddy's BarLog, at
//! `%LOCALAPPDATA%\QC Buddy\QCBuddy.log` (previous file kept as
//! `QCBuddy.log.1`). Logging never fails the caller.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_SIZE: u64 = 512 * 1024;

#[derive(Clone)]
pub struct Log {
    inner: Arc<Mutex<Sink>>,
}

enum Sink {
    File(PathBuf),
    Memory(Vec<String>),
    /// Also echo to stderr (the CLI's --verbose).
    FileAndStderr(PathBuf),
}

/// `%LOCALAPPDATA%\QC Buddy`, or the temp dir if LOCALAPPDATA is unset.
pub fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("QC Buddy")
}

pub fn default_path() -> PathBuf {
    data_dir().join("QCBuddy.log")
}

impl Log {
    pub fn file(path: impl Into<PathBuf>) -> Log {
        Log { inner: Arc::new(Mutex::new(Sink::File(path.into()))) }
    }

    pub fn file_and_stderr(path: impl Into<PathBuf>) -> Log {
        Log { inner: Arc::new(Mutex::new(Sink::FileAndStderr(path.into()))) }
    }

    /// Keeps lines in memory (tests).
    pub fn memory() -> Log {
        Log { inner: Arc::new(Mutex::new(Sink::Memory(Vec::new()))) }
    }

    pub fn path(&self) -> Option<PathBuf> {
        match &*self.inner.lock().ok()? {
            Sink::File(p) | Sink::FileAndStderr(p) => Some(p.clone()),
            Sink::Memory(_) => None,
        }
    }

    /// Lines written to a memory log, without timestamps.
    pub fn lines(&self) -> Vec<String> {
        match self.inner.lock().as_deref() {
            Ok(Sink::Memory(lines)) => lines.clone(),
            _ => Vec::new(),
        }
    }

    pub fn write(&self, message: impl AsRef<str>) {
        let message = message.as_ref();
        let Ok(mut sink) = self.inner.lock() else { return };
        match &mut *sink {
            Sink::Memory(lines) => lines.push(message.to_string()),
            Sink::File(path) => append(path, message),
            Sink::FileAndStderr(path) => {
                eprintln!("  · {message}");
                append(path, message);
            }
        }
    }
}

fn append(path: &PathBuf, message: &str) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let line = format!("{} {message}\n", iso8601(now));
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(path).map(|m| m.len() > MAX_SIZE).unwrap_or(false) {
        // Keep the previous file so history before a bug survives.
        let mut previous = path.clone().into_os_string();
        previous.push(".1");
        let _ = std::fs::remove_file(&previous);
        let _ = std::fs::rename(path, &previous);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

/// UTC timestamp like ISO8601DateFormatter: 2026-11-25T19:32:13Z.
pub fn iso8601(unix_seconds: u64) -> String {
    let days = (unix_seconds / 86_400) as i64;
    let rem = unix_seconds % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso8601(1_798_761_599), "2026-12-31T23:59:59Z");
    }

    #[test]
    fn file_log_appends_and_rotates() {
        let dir = std::env::temp_dir().join(format!("qcbuddy-log-test-{}", std::process::id()));
        let path = dir.join("QCBuddy.log");
        let _ = std::fs::remove_dir_all(&dir);
        let log = Log::file(&path);
        log.write("first");
        log.write("second");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.lines().count() == 2 && text.ends_with(" second\n"));

        std::fs::write(&path, vec![b'x'; MAX_SIZE as usize + 1]).unwrap();
        log.write("after rotation");
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
        assert!(dir.join("QCBuddy.log.1").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn memory_log() {
        let log = Log::memory();
        log.write("peers: a:on");
        assert_eq!(log.lines(), ["peers: a:on"]);
    }
}
