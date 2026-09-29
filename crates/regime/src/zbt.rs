//! Zweig Breadth Thrust (ZBT) — curated reported history.
//!
//! **Deliberately no math engine.** A ZBT is computed from daily NYSE
//! advancing/declining issues, and there is no free, reliable feed for that
//! breadth data (FRED carries `UPADNS`/`DOWADNS` but needs an API key and is a
//! research database, not a scan-time feed; everything else is paywalled or
//! scrape-only). Faking breadth data would be worse than useless, so this
//! module does not compute thrusts at all.
//!
//! Instead, reported events are the authority: every ZBT since WWII made the
//! financial press (roughly 20 occurrences — Carson/Detrick/Ned Davis counts
//! vary slightly by methodology). This module ships the curated,
//! source-attributed history as embedded JSON (`data/zbt_history.json`) and
//! answers "when was the last thrust?" — the scan's research step keeps it
//! current by appending newly reported thrusts to `docs/regime-events.jsonl`
//! (see [`crate::events`]).
//!
//! # Residual gap, stated honestly
//!
//! A thrust with zero media coverage would be missed. Historically
//! implausible — a ~1-in-4-years event that technicians watch for — but not
//! impossible, so it is recorded here rather than hand-waved.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::RegimeError;

/// One historically reported Zweig Breadth Thrust.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZbtEvent {
    /// Trigger date: `YYYY-MM-DD` (day precision) or `YYYY-MM` / `YYYY`.
    pub date: String,
    /// One of `"day"`, `"month"`, `"year"`.
    pub date_precision: String,
    /// Who reported it.
    pub source: String,
    /// Link to the report, when available.
    pub url: Option<String>,
    /// Context (counts, forward returns, caveats).
    pub note: Option<String>,
}

/// The embedded curated history, newest first.
static ZBT_HISTORY_JSON: &str = include_str!("../data/zbt_history.json");

/// Parse the embedded history. Fails only if the shipped data file is
/// malformed (a build-time data bug — fail closed, never silently empty).
pub fn history() -> Result<Vec<ZbtEvent>, RegimeError> {
    serde_json::from_str(ZBT_HISTORY_JSON)
        .map_err(|e| RegimeError::ZbtHistory(format!("embedded data malformed: {e}")))
}

/// The most recent thrust on or before `date` (`YYYY-MM-DD`).
#[must_use]
pub fn last_thrust_on_or_before(history: &[ZbtEvent], date: &str) -> Option<ZbtEvent> {
    history
        .iter()
        .filter(|e| e.date.as_str() <= date)
        .max_by_key(|e| e.date.clone())
        .cloned()
}

/// Calendar days from the last day-precision thrust on/before `as_of` to
/// `as_of`. Returns `None` when there is no thrust on/before `as_of`, when the
/// most recent one lacks day precision (a days-since number would be
/// fabricated), or when either date fails to parse.
#[must_use]
pub fn days_since_thrust(history: &[ZbtEvent], as_of: &str) -> Option<i64> {
    let last = last_thrust_on_or_before(history, as_of)?;
    if last.date_precision != "day" {
        return None;
    }
    let thrust = NaiveDate::parse_from_str(&last.date, "%Y-%m-%d").ok()?;
    let as_of = NaiveDate::parse_from_str(as_of, "%Y-%m-%d").ok()?;
    Some((as_of - thrust).num_days())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hist() -> Vec<ZbtEvent> {
        history().unwrap()
    }

    #[test]
    fn embedded_history_parses_and_is_newest_first() {
        let h = hist();
        assert!(h.len() >= 3);
        assert_eq!(h[0].date, "2025-04-24");
        assert!(h.windows(2).all(|w| w[0].date >= w[1].date));
    }

    #[test]
    fn last_thrust_lookup() {
        let h = hist();
        let e = last_thrust_on_or_before(&h, "2024-01-01").unwrap();
        assert_eq!(e.date, "2023-11-03");
        assert!(last_thrust_on_or_before(&h, "1960-01-01").is_none());
    }

    #[test]
    fn days_since_thrust_day_precision() {
        let h = hist();
        assert_eq!(days_since_thrust(&h, "2025-04-24"), Some(0));
        // 2025-04-24 -> 2025-05-24 is 30 days.
        assert_eq!(days_since_thrust(&h, "2025-05-24"), Some(30));
    }

    #[test]
    fn days_since_thrust_refuses_imprecise_dates() {
        // A history whose newest entry is month-precision: no honest
        // days-since number exists.
        let h = vec![ZbtEvent {
            date: "2009-03".to_string(),
            date_precision: "month".to_string(),
            source: "test".to_string(),
            url: None,
            note: None,
        }];
        assert_eq!(days_since_thrust(&h, "2009-06-01"), None);
    }
}
