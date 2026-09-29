//! Append-only reported-regime-events log.
//!
//! File: `docs/regime-events.jsonl` (repo root `docs/`). One JSON object per
//! line, appended by the scan's research step — never edited in place:
//!
//! ```json
//! {"date":"2025-04-24","event":"ZBT","detail":"Zweig Breadth Thrust triggered","source":"Carson Investment Research / Ryan Detrick","url":"https://...","status":"ConfirmedUptrend"}
//! ```
//!
//! - `event`: `"ZBT"` | `"FTD"` | `"MARKET_STATUS"`.
//! - `detail`: human-readable description of what was reported.
//! - `source` / `url`: who reported it and where (the provenance that makes
//!   reported events trustworthy).
//! - `status` (optional, for `MARKET_STATUS`): normalized regime —
//!   `"Correction"`, `"AttemptedRally"`, or `"ConfirmedUptrend"` — so the
//!   merge step does not have to parse prose. IBD's "Uptrend Under Pressure"
//!   maps to `ConfirmedUptrend` with `weakening: true` in the merged output.
//!
//! A missing log file is **not** an error (the scan may not have created one
//! yet) — it reads as an empty log. A malformed line **is** an error: the log
//! is ours, and silent corruption is worse than failing closed.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{RegimeError, RegimeState};

/// The kind of reported regime event. Wire form is SCREAMING_SNAKE_CASE:
/// `"ZBT"`, `"FTD"`, `"MARKET_STATUS"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventKind {
    /// A reported Zweig Breadth Thrust.
    Zbt,
    /// A reported follow-through day (e.g. IBD calling one out).
    Ftd,
    /// A published market-status call (IBD Big Picture, BofA technicals,
    /// Carson/Detrick, SentimenTrader, ...).
    MarketStatus,
}

/// One line of the events log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeEvent {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub event: EventKind,
    pub detail: String,
    pub source: String,
    pub url: Option<String>,
    /// Normalized regime for `MARKET_STATUS` events.
    pub status: Option<String>,
}

/// Read and parse the events log. Missing file → empty vec (not an error).
/// Malformed line → [`RegimeError::EventsLog`].
pub fn read_events_log(path: &Path) -> Result<Vec<RegimeEvent>, RegimeError> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => {
            return Err(RegimeError::EventsLog(format!(
                "cannot read {}: {e}",
                path.display()
            )));
        }
    };
    let mut events = Vec::new();
    for (n, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let event: RegimeEvent = serde_json::from_str(line)
            .map_err(|e| RegimeError::EventsLog(format!("line {}: malformed JSON: {e}", n + 1)))?;
        events.push(event);
    }
    events.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(events)
}

/// Normalize a reported status string to a [`RegimeState`]. Returns `None`
/// for unrecognized prose — the caller records it as unmapped, never guesses.
#[must_use]
pub fn map_reported_status(raw: &str) -> Option<RegimeState> {
    match raw.trim().to_lowercase().as_str() {
        "confirmeduptrend" | "confirmed uptrend" | "confirmed_uptrend" => {
            Some(RegimeState::ConfirmedUptrend)
        }
        "uptrend under pressure" | "uptrend_under_pressure" => Some(RegimeState::ConfirmedUptrend),
        "attemptedrally" | "attempted rally" | "rally attempt" | "rally_attempt" => {
            Some(RegimeState::AttemptedRally)
        }
        "correction" | "market in correction" | "market_in_correction" => {
            Some(RegimeState::Correction)
        }
        _ => None,
    }
}

/// The latest `MARKET_STATUS` event, if any.
#[must_use]
pub fn latest_market_status(events: &[RegimeEvent]) -> Option<&RegimeEvent> {
    events
        .iter()
        .filter(|e| e.event == EventKind::MarketStatus)
        .max_by_key(|e| e.date.clone())
}

/// The latest reported `FTD` event, if any.
#[must_use]
pub fn latest_reported_ftd(events: &[RegimeEvent]) -> Option<&RegimeEvent> {
    events
        .iter()
        .filter(|e| e.event == EventKind::Ftd)
        .max_by_key(|e| e.date.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_log(lines: &[&str]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "thales-regime-test-{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut f = std::fs::File::create(&path).unwrap();
        for line in lines {
            writeln!(f, "{line}").unwrap();
        }
        path
    }

    fn remove_log(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parses_valid_log_and_picks_latest_status() {
        let path = write_log(&[
            r#"{"date":"2026-09-20","event":"MARKET_STATUS","detail":"IBD: Market in Correction","source":"IBD Big Picture","url":null,"status":"Correction"}"#,
            r#"{"date":"2026-09-27","event":"MARKET_STATUS","detail":"IBD: Confirmed Uptrend","source":"IBD Big Picture","url":null,"status":"ConfirmedUptrend"}"#,
            r#"{"date":"2026-09-27","event":"ZBT","detail":"thrust reported","source":"Carson","url":null,"status":null}"#,
        ]);
        let events = read_events_log(&path).unwrap();
        assert_eq!(events.len(), 3);
        let latest = latest_market_status(&events).unwrap();
        assert_eq!(latest.date, "2026-09-27");
        assert_eq!(
            map_reported_status(latest.status.as_deref().unwrap()),
            Some(RegimeState::ConfirmedUptrend)
        );
        remove_log(&path);
    }

    #[test]
    fn missing_file_is_empty_not_error() {
        let events =
            read_events_log(Path::new("/tmp/thales-test-no-such-log-12345.jsonl")).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn malformed_line_fails_closed() {
        let path = write_log(&[
            r#"{"date":"2026-09-27","event":"ZBT","detail":"ok","source":"x","url":null,"status":null}"#,
            r#"{"date": broken"#,
        ]);
        let err = read_events_log(&path).unwrap_err();
        assert!(matches!(err, RegimeError::EventsLog(_)));
        remove_log(&path);
    }

    #[test]
    fn status_mapping() {
        assert_eq!(
            map_reported_status("Confirmed Uptrend"),
            Some(RegimeState::ConfirmedUptrend)
        );
        assert_eq!(
            map_reported_status("Uptrend Under Pressure"),
            Some(RegimeState::ConfirmedUptrend)
        );
        assert_eq!(
            map_reported_status("Rally Attempt"),
            Some(RegimeState::AttemptedRally)
        );
        assert_eq!(
            map_reported_status("Market in Correction"),
            Some(RegimeState::Correction)
        );
        assert_eq!(map_reported_status("to the moon"), None);
    }
}
