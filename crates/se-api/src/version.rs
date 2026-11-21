//! Which Rosson version EquiEvents identifies itself as (`rosson_version` in uploads,
//! `app_version` in queries).
//!
//! Decided (2026-11-13): the latest Rosson version, found the way Rosson's own start-up update
//! check finds it. Rosson uses electron-updater with the S3 feed configured in
//! `Rosson.app/Contents/Resources/app-update.yml` (bucket `production-rosson`): it reads
//! `latest.yml` (Windows) or `latest-mac.yml` (macOS), whose `version:` line is the latest release.
//!
//! - Read at start, when online; the version never goes backwards (the highest seen is kept).
//! - Anything but a plain version number (`3.3.15`) in the feed is ignored.
//! - Offline or never read: the last version seen, else [`FALLBACK`].

use std::cmp::Ordering;

/// Rosson's update feed for Windows (and for the web server).
pub const FEED_WINDOWS: &str = "https://production-rosson.s3.amazonaws.com/latest.yml";
/// Rosson's update feed for macOS.
pub const FEED_MACOS: &str = "https://production-rosson.s3.amazonaws.com/latest-mac.yml";
/// The version sent when no feed was ever read: Rosson's release at the time of writing.
pub const FALLBACK: &str = "3.3.15";

/// The feed this platform's Rosson would read.
pub fn feed_url(macos: bool) -> &'static str {
    if macos { FEED_MACOS } else { FEED_WINDOWS }
}

/// The version in a feed (`version: 3.3.15`), if it is a plain version number.
pub fn from_feed(yaml: &str) -> Option<String> {
    let line = yaml.lines().find_map(|l| l.strip_prefix("version:"))?;
    let version = line.trim().trim_matches(|c| c == '\'' || c == '"');
    parse(version).map(|_| version.to_string())
}

/// The version to identify as: the newest of what the feed says now, what was seen before, and
/// the fallback.
pub fn current(from_feed_now: Option<&str>, seen_before: Option<&str>) -> String {
    [from_feed_now, seen_before, Some(FALLBACK)].into_iter().flatten().filter(|v| parse(v).is_some()).max_by(|a, b| compare(a, b)).unwrap_or(FALLBACK).to_string()
}

fn parse(version: &str) -> Option<Vec<u32>> {
    let parts: Option<Vec<u32>> = version.split('.').map(|p| if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) { None } else { p.parse().ok() }).collect();
    parts.filter(|p| p.len() == 3)
}

/// Compare two plain version numbers (3.3.15 < 3.4.0 < 3.10.0).
pub fn compare(a: &str, b: &str) -> Ordering {
    parse(a).cmp(&parse(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_line() {
        assert_eq!(from_feed("version: 3.3.15\nfiles:\n  - url: x\n").as_deref(), Some("3.3.15"));
        assert_eq!(from_feed("version: '3.4.0'\n").as_deref(), Some("3.4.0"));
        assert_eq!(from_feed("version: 3.4.0-beta.1\n"), None);
        assert_eq!(from_feed("<Error>AccessDenied</Error>"), None);
    }

    #[test]
    fn never_goes_backwards() {
        assert_eq!(current(Some("3.3.16"), None), "3.3.16");
        assert_eq!(current(Some("3.3.14"), Some("3.3.16")), "3.3.16");
        assert_eq!(current(None, None), FALLBACK);
        assert_eq!(current(Some("3.10.0"), Some("3.9.9")), "3.10.0");
        assert_eq!(current(Some("garbage"), None), FALLBACK);
    }
}
