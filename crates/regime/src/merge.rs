//! Merge: one regime reading from computed FTD + reported events + ZBT history.
//!
//! This is the unit the coordinator consumes. The merge rule is deliberately
//! boring and documented here so a reading is always reproducible:
//!
//! 1. The **computed** FTD state machine is the primary reading — it is
//!    deterministic and always available when bars exist.
//! 2. The latest reported `MARKET_STATUS` event is the **cross-check**.
//!    - If it agrees with computed → `basis: both`.
//!    - If it disagrees → the merged regime stays on computed,
//!      `basis: computed`, `agreement: false`, and the disagreement is
//!      spelled out in `notes` (a human, not this crate, resolves it).
//!    - If computed is `Unknown` but a reported status exists → the reported
//!      status stands, `basis: reported`.
//! 3. ZBT history is **context, not a state**: the report always carries the
//!    last thrust date, days-since, and source. A thrust within the last 90
//!    days is flagged because the 6–12-month forward record after thrusts is
//!    the reason anyone watches them.
//!
//! Nothing here emits a [`TradeIntent`](contracts::TradeIntent), and nothing
//! here can approve one. Regime context informs; it never decides.

use serde::{Deserialize, Serialize};

use crate::events::{RegimeEvent, latest_market_status, latest_reported_ftd, map_reported_status};
use crate::ftd::FtdResult;
use crate::zbt::ZbtEvent;
use crate::{DetectorBasis, RegimeError, RegimeState};

/// The reported-events half of the merged reading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportedSection {
    /// The latest reported market status, verbatim detail + source.
    pub market_status: Option<ReportedStatus>,
    /// The latest reported follow-through day, if any.
    pub reported_ftd: Option<ReportedFtd>,
    /// Number of events read from the log.
    pub events_seen: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportedStatus {
    pub date: String,
    pub detail: String,
    pub source: String,
    pub url: Option<String>,
    /// Normalized to our regime vocabulary; None when the prose did not
    /// match a known status (recorded, never guessed).
    pub mapped: Option<RegimeState>,
    /// True when the raw status was "Uptrend Under Pressure" — still an
    /// uptrend, but weakening.
    pub weakening: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportedFtd {
    pub date: String,
    pub detail: String,
    pub source: String,
    pub url: Option<String>,
}

/// The ZBT context half of the merged reading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZbtSection {
    /// Most recent thrust on/before `as_of`.
    pub last_thrust: Option<ZbtEvent>,
    /// Days from that thrust to `as_of` (None when the newest entry lacks
    /// day precision — we do not fabricate a number).
    pub days_since_thrust: Option<i64>,
    /// True when the last thrust was within 90 days of `as_of`.
    pub within_90d_window: bool,
    /// Number of thrusts in the curated history.
    pub history_count: usize,
}

/// The merged regime report — what the coordinator consumes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeReport {
    pub detector: String,
    pub regime: RegimeState,
    /// `YYYY-MM-DD` of the reading (last bar, else latest event, else "").
    pub as_of: String,
    pub basis: DetectorBasis,
    pub ftd: FtdResult,
    pub reported: ReportedSection,
    pub zbt: ZbtSection,
    /// None when one side was unavailable for comparison.
    pub agreement: Option<bool>,
    pub notes: Vec<String>,
}

/// How far back a thrust still counts as "recent context".
pub const ZBT_RECENT_WINDOW_DAYS: i64 = 90;

/// Merge the computed FTD result, reported events, and ZBT history.
#[must_use]
pub fn merge(ftd: FtdResult, events: &[RegimeEvent], zbt_history: &[ZbtEvent]) -> RegimeReport {
    let mut notes: Vec<String> = Vec::new();

    let as_of = if !ftd.as_of.is_empty() {
        ftd.as_of.clone()
    } else {
        events
            .iter()
            .map(|e| e.date.clone())
            .max()
            .unwrap_or_default()
    };

    // ---- reported section ----
    let market_status = latest_market_status(events).map(|e| {
        let mapped = e.status.as_deref().and_then(map_reported_status);
        if e.status.is_some() && mapped.is_none() {
            notes.push(format!(
                "reported market status on {} did not match a known status (detail: {:?}); recorded as unmapped",
                e.date, e.detail
            ));
        }
        let weakening = e
            .status
            .as_deref()
            .is_some_and(|s| s.trim().eq_ignore_ascii_case("Uptrend Under Pressure"));
        ReportedStatus {
            date: e.date.clone(),
            detail: e.detail.clone(),
            source: e.source.clone(),
            url: e.url.clone(),
            mapped,
            weakening,
        }
    });
    let reported_ftd = latest_reported_ftd(events).map(|e| ReportedFtd {
        date: e.date.clone(),
        detail: e.detail.clone(),
        source: e.source.clone(),
        url: e.url.clone(),
    });
    let reported = ReportedSection {
        market_status,
        reported_ftd,
        events_seen: events.len(),
    };

    // ---- merge rule ----
    let computed = ftd.state;
    let reported_state = reported.market_status.as_ref().and_then(|s| s.mapped);
    let (regime, basis, agreement) = match (computed, reported_state) {
        (RegimeState::Unknown, Some(r)) => {
            notes.push(format!(
                "no computed reading ({}); standing on reported status",
                ftd.details.notes.join("; ")
            ));
            (r, DetectorBasis::Reported, None)
        }
        (RegimeState::Unknown, None) => (RegimeState::Unknown, DetectorBasis::Computed, None),
        (c, Some(r)) if c == r => (c, DetectorBasis::Both, Some(true)),
        (c, Some(r)) => {
            notes.push(format!(
                "computed ({}) and reported ({}) disagree; holding computed — resolve by hand",
                c.as_str(),
                r.as_str()
            ));
            (c, DetectorBasis::Computed, Some(false))
        }
        (c, None) => (c, DetectorBasis::Computed, None),
    };

    // ---- ZBT section ----
    let last_thrust = crate::zbt::last_thrust_on_or_before(zbt_history, &as_of);
    let days_since_thrust = if as_of.is_empty() {
        None
    } else {
        crate::zbt::days_since_thrust(zbt_history, &as_of)
    };
    let within_90d_window =
        days_since_thrust.is_some_and(|d| (0..=ZBT_RECENT_WINDOW_DAYS).contains(&d));
    if within_90d_window && let Some(ref thrust) = last_thrust {
        notes.push(format!(
            "Zweig Breadth Thrust on {} ({}d ago) is inside the 90-day post-thrust window — historically strong 6-12m forward returns; context only, not a signal",
            thrust.date,
            days_since_thrust.unwrap_or(-1)
        ));
    }
    let zbt = ZbtSection {
        last_thrust,
        days_since_thrust,
        within_90d_window,
        history_count: zbt_history.len(),
    };

    // A reported FTD that our machine missed (or vice versa) is worth a line.
    if let Some(ref rf) = reported.reported_ftd
        && ftd.details.follow_through_date.is_none()
    {
        notes.push(format!(
            "reported FTD on {} ({}) has no computed counterpart in this bar window",
            rf.date, rf.source
        ));
    }

    RegimeReport {
        detector: "REGIME".to_string(),
        regime,
        as_of,
        basis,
        ftd,
        reported,
        zbt,
        agreement,
        notes,
    }
}

/// Convenience: build the report or return the typed error.
pub fn regime_report(
    ftd: Result<FtdResult, RegimeError>,
    events: Result<Vec<RegimeEvent>, RegimeError>,
    zbt_history: Result<Vec<ZbtEvent>, RegimeError>,
) -> Result<RegimeReport, RegimeError> {
    Ok(merge(ftd?, &events?, &zbt_history?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventKind;
    use crate::ftd::{FtdConfig, FtdDetails, detect};
    use contracts::{Bar, BarSeries};

    fn empty_ftd(as_of: &str) -> FtdResult {
        let s = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![],
        };
        let mut r = detect(&s, &FtdConfig::default()).unwrap();
        r.as_of = as_of.to_string();
        r
    }

    fn confirmed_ftd() -> FtdResult {
        // Reuse the ftd module's confirmation path via synthetic bars.
        let t0: i64 = 1_767_571_200_000; // 2026-01-05T00:00:00Z
        let day: i64 = 86_400_000;
        let mut closes: Vec<f64> = (0..25).map(|i| 500.0 - i as f64 * 2.0).collect();
        let day1 = closes[24] * 1.004;
        closes.push(day1);
        closes.push(day1 * 1.002);
        closes.push(day1 * 1.001);
        closes.push(day1 * 1.003);
        closes.push(day1 * 1.003 * 1.015);
        let bars: Vec<Bar> = closes
            .iter()
            .enumerate()
            .map(|(i, &c)| Bar {
                symbol: "SPY".to_string(),
                market: "equities".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: t0 + i as i64 * day,
                open: c * 0.999,
                high: c * 1.002,
                low: c * 0.998,
                close: c,
                volume: if i == 29 { 2_000_000.0 } else { 1_000_000.0 },
                adjusted_close: None,
            })
            .collect();
        detect(
            &BarSeries {
                schema_version: "v0".to_string(),
                bars,
            },
            &FtdConfig::default(),
        )
        .unwrap()
    }

    fn status_event(date: &str, status: &str) -> RegimeEvent {
        RegimeEvent {
            date: date.to_string(),
            event: EventKind::MarketStatus,
            detail: format!("IBD: {status}"),
            source: "IBD Big Picture".to_string(),
            url: None,
            status: Some(status.to_string()),
        }
    }

    fn zbt_hist() -> Vec<ZbtEvent> {
        crate::zbt::history().unwrap()
    }

    #[test]
    fn agreement_yields_both() {
        let ftd = confirmed_ftd();
        let events = vec![status_event("2026-02-04", "ConfirmedUptrend")];
        let r = merge(ftd, &events, &zbt_hist());
        assert_eq!(r.regime, RegimeState::ConfirmedUptrend);
        assert_eq!(r.basis, DetectorBasis::Both);
        assert_eq!(r.agreement, Some(true));
    }

    #[test]
    fn disagreement_holds_computed_and_flags() {
        let ftd = confirmed_ftd();
        let events = vec![status_event("2026-02-04", "Correction")];
        let r = merge(ftd, &events, &zbt_hist());
        assert_eq!(r.regime, RegimeState::ConfirmedUptrend);
        assert_eq!(r.basis, DetectorBasis::Computed);
        assert_eq!(r.agreement, Some(false));
        assert!(r.notes.iter().any(|n| n.contains("disagree")));
    }

    #[test]
    fn no_events_is_computed_only() {
        let ftd = confirmed_ftd();
        let r = merge(ftd, &[], &zbt_hist());
        assert_eq!(r.basis, DetectorBasis::Computed);
        assert_eq!(r.agreement, None);
        assert_eq!(r.reported.events_seen, 0);
    }

    #[test]
    fn empty_bars_with_reported_status_stands_on_reported() {
        let ftd = empty_ftd("2026-09-28");
        let events = vec![status_event("2026-09-27", "Correction")];
        let r = merge(ftd, &events, &zbt_hist());
        assert_eq!(r.regime, RegimeState::Correction);
        assert_eq!(r.basis, DetectorBasis::Reported);
    }

    #[test]
    fn zbt_context_carried() {
        let ftd = empty_ftd("2025-05-24");
        let r = merge(ftd, &[], &zbt_hist());
        assert_eq!(r.zbt.days_since_thrust, Some(30));
        assert!(r.zbt.within_90d_window);
        assert_eq!(r.zbt.last_thrust.unwrap().date, "2025-04-24");
        assert!(r.notes.iter().any(|n| n.contains("90-day")));
    }

    #[test]
    fn details_struct_is_constructible() {
        // Guards the FtdDetails shape used by merge (distribution_days stub).
        let d = FtdDetails {
            rally_day: None,
            day1_date: None,
            day1_low: None,
            follow_through_date: None,
            follow_through_gain_pct: None,
            follow_through_volume_ratio: None,
            follow_through_low: None,
            distribution_days: None,
            notes: vec![],
        };
        assert!(d.distribution_days.is_none());
    }
}
