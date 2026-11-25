//! The update feed the tray app's updater reads, for the CLI's
//! `qcb update`: is there a newer QC Buddy than this one?

use std::cmp::Ordering;

/// What installed copies read on Windows: the app's updater endpoint
/// (`…/latest-{{target}}.json` in tauri.conf.json) with the target filled in.
pub const WINDOWS_FEED_URL: &str =
    "https://github.com/hu13/BoseBuddy/releases/download/qcbuddy-latest/latest-windows.json";

/// The version this build is (the app and core are released together).
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// `x.y.z` as numbers; a leading `v` is allowed.
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.trim().trim_start_matches('v').splitn(3, '.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    // Ignore any pre-release/build suffix: "3-beta" → 3.
    let patch_text = parts.next()?;
    let digits: String = patch_text.chars().take_while(char::is_ascii_digit).collect();
    Some((major, minor, digits.parse().ok()?))
}

pub fn compare(a: &str, b: &str) -> Option<Ordering> {
    Some(parse_version(a)?.cmp(&parse_version(b)?))
}

/// The version a feed offers.
pub fn feed_version(json: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("the update feed isn't valid JSON ({e})"))?;
    let version = value["version"].as_str().ok_or("the update feed has no version")?;
    parse_version(version).ok_or_else(|| format!("the update feed has an odd version \"{version}\""))?;
    Ok(version.to_string())
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    UpToDate,
    Newer(String),
    /// The feed offers an older version than this one (a local or pre-release build).
    Ahead(String),
}

pub fn verdict(current: &str, feed: &str) -> Verdict {
    match compare(feed, current) {
        Some(Ordering::Greater) => Verdict::Newer(feed.to_string()),
        Some(Ordering::Less) => Verdict::Ahead(feed.to_string()),
        _ => Verdict::UpToDate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("0.1.2"), Some((0, 1, 2)));
        assert_eq!(parse_version("v1.10.0"), Some((1, 10, 0)));
        assert_eq!(parse_version("0.2.3-beta"), Some((0, 2, 3)));
        assert_eq!(parse_version("0.1"), None);
        assert_eq!(compare("0.1.10", "0.1.9"), Some(Ordering::Greater), "numbers, not text");
    }

    #[test]
    fn verdicts() {
        assert_eq!(verdict("0.1.2", "0.1.3"), Verdict::Newer("0.1.3".into()));
        assert_eq!(verdict("0.1.2", "0.1.2"), Verdict::UpToDate);
        assert_eq!(verdict("0.1.3", "0.1.2"), Verdict::Ahead("0.1.2".into()));
    }

    #[test]
    fn reads_the_feed_the_release_workflow_writes() {
        let feed = r#"{"version":"0.1.3","notes":"x","pub_date":"2026-11-25T23:00:00Z",
            "platforms":{"windows-x86_64":{"signature":"s","url":"https://example.com/a.exe"}}}"#;
        assert_eq!(feed_version(feed), Ok("0.1.3".into()));
        assert!(feed_version("<html>Not Found</html>").unwrap_err().contains("isn't valid JSON"));
        assert!(feed_version(r#"{"notes":"x"}"#).unwrap_err().contains("no version"));
    }

    /// The CLI must read the same feed the tray app's updater does.
    #[test]
    fn same_feed_as_the_app() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../../app/src-tauri/tauri.conf.json")).unwrap();
        let endpoint = conf["plugins"]["updater"]["endpoints"][0].as_str().unwrap();
        assert_eq!(endpoint.replace("{{target}}", "windows"), WINDOWS_FEED_URL);
        assert_eq!(conf["version"], CURRENT_VERSION, "app and core versions move together");
    }
}
