//! Follow-Through Day (FTD) regime state machine (O'Neil CAN SLIM).
//!
//! A deterministic, paywall-free regime detector computed from daily index
//! bars + volume. It tracks the classic O'Neil sequence:
//!
//! ```text
//! Correction → AttemptedRally (Day 1) → ConfirmedUptrend (follow-through day)
//!      ↑               │
//!      └───────────────┘  (Day-1 low undercut, or no follow-through by day 10)
//! ```
//!
//! ConfirmedUptrend → Correction when price undercuts the follow-through
//! day's low (v1 invalidation rule).
//!
//! # Parameters (see [`FtdConfig`])
//!
//! - **Day 1**: a bar that closes up, where the trailing-20-bar low was set
//!   within the last 3 bars (the "low, then up-close" that starts a rally
//!   attempt).
//! - **Follow-through day**: rally days 3–10 (classic window is 4–7, accepted
//!   3–10), a gain of at least 1% on volume greater than the prior day.
//! - **Failure**: any bar in the attempt that undercuts the Day-1 low, or no
//!   follow-through by the end of rally day 10 (stalled attempt).
//!
//! # Design notes
//!
//! - Distribution-day counting (the other half of O'Neil's framework) is
//!   deliberately out of scope for v1; see the `TODO` in
//!   [`FtdDetails`] and `docs/regime-detectors.md`. The state machine is
//!   structured so a distribution counter can be added without changing the
//!   confirmation logic.
//! - The machine consumes the trailing `max_bars` bars; earlier history is
//!   ignored, so readings are reproducible from the input alone.

use chrono::DateTime;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

use crate::{RegimeError, RegimeState};

/// Tunable parameters of the FTD state machine. Defaults encode the classic
/// O'Neil rules; the struct exists so the coordinator (or tests) can state
/// exactly which rules produced a reading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtdConfig {
    /// Trailing window (bars) used to define a "fresh low" for Day 1.
    pub low_lookback: usize,
    /// How recently (bars) the fresh low must have been set for a Day-1
    /// trigger: 3 means the low was set on the prior bar or up to 2 bars
    /// earlier.
    pub low_recency: usize,
    /// Minimum close-to-close gain for a follow-through day (0.01 = 1%).
    pub min_gain_pct: f64,
    /// First eligible rally day for a follow-through (classic window 4–7,
    /// accepted 3–10).
    pub ft_window_start: u32,
    /// Last eligible rally day for a follow-through.
    pub ft_window_end: u32,
    /// Trailing bars analyzed; older history is ignored.
    pub max_bars: usize,
}

impl Default for FtdConfig {
    fn default() -> Self {
        Self {
            low_lookback: 20,
            low_recency: 3,
            min_gain_pct: 0.01,
            ft_window_start: 3,
            ft_window_end: 10,
            max_bars: 252,
        }
    }
}

/// Machine-readable detail behind an FTD reading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtdDetails {
    /// Current rally-attempt day count (None unless in AttemptedRally).
    pub rally_day: Option<u32>,
    /// Date (YYYY-MM-DD) of the Day-1 bar, if an attempt is/was active.
    pub day1_date: Option<String>,
    /// The Day-1 low — undercutting it fails the attempt.
    pub day1_low: Option<f64>,
    /// Date of the confirming follow-through day, if confirmed.
    pub follow_through_date: Option<String>,
    /// Close-to-close gain of the follow-through day, as a fraction.
    pub follow_through_gain_pct: Option<f64>,
    /// Follow-through day volume / prior day volume.
    pub follow_through_volume_ratio: Option<f64>,
    /// Low of the follow-through bar — undercutting it ends the confirmed
    /// uptrend (v1 invalidation rule).
    pub follow_through_low: Option<f64>,
    /// TODO(v2): distribution-day count for the current/confirmed rally.
    /// O'Neil's framework tracks institutional selling via distribution days
    /// (a down day on higher volume); 5+ within a window ends the uptrend.
    /// Not yet implemented — always None in v1.
    pub distribution_days: Option<u32>,
    /// Human-readable notes (stalls, invalidations, data caveats).
    pub notes: Vec<String>,
}

/// The FTD reading: a regime state plus its evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtdResult {
    pub detector: String,
    pub state: RegimeState,
    /// YYYY-MM-DD of the last input bar ("" when input was empty).
    pub as_of: String,
    pub symbol: String,
    pub details: FtdDetails,
}

/// Run the FTD state machine over a [`BarSeries`] (ascending by time).
///
/// # Errors
///
/// - [`RegimeError::BadInput`] on non-finite/non-positive prices or
///   negative/non-finite volume — the detector fails closed rather than
///   reading a corrupted tape.
/// - Empty input is **not** an error: it yields `state: Unknown` with a note.
pub fn detect(series: &BarSeries, config: &FtdConfig) -> Result<FtdResult, RegimeError> {
    let bars: Vec<&contracts::Bar> = if series.bars.len() > config.max_bars {
        series.bars[series.bars.len() - config.max_bars..]
            .iter()
            .collect()
    } else {
        series.bars.iter().collect()
    };

    if bars.is_empty() {
        return Ok(FtdResult {
            detector: "FTD".to_string(),
            state: RegimeState::Unknown,
            as_of: String::new(),
            symbol: String::new(),
            details: FtdDetails {
                rally_day: None,
                day1_date: None,
                day1_low: None,
                follow_through_date: None,
                follow_through_gain_pct: None,
                follow_through_volume_ratio: None,
                follow_through_low: None,
                distribution_days: None,
                notes: vec!["empty input: no bars to evaluate".to_string()],
            },
        });
    }

    for (i, bar) in bars.iter().enumerate() {
        if !bar.open.is_finite()
            || !bar.high.is_finite()
            || !bar.low.is_finite()
            || !bar.close.is_finite()
            || !bar.volume.is_finite()
            || bar.close <= 0.0
            || bar.low <= 0.0
            || bar.high <= 0.0
            || bar.volume < 0.0
        {
            return Err(RegimeError::BadInput(
                i,
                "non-finite or invalid OHLCV".to_string(),
            ));
        }
    }

    let as_of = bar_date(bars[bars.len() - 1].timestamp_unix_ms);
    let symbol = bars[0].symbol.clone();

    let mut details = FtdDetails {
        rally_day: None,
        day1_date: None,
        day1_low: None,
        follow_through_date: None,
        follow_through_gain_pct: None,
        follow_through_volume_ratio: None,
        follow_through_low: None,
        distribution_days: None,
        notes: Vec::new(),
    };

    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        Correction,
        AttemptedRally,
        ConfirmedUptrend,
    }
    let mut phase = Phase::Correction;
    let mut rally_day: u32 = 0;
    let mut day1_low = 0.0;

    for i in 1..bars.len() {
        let prev = bars[i - 1];
        let bar = bars[i];
        match phase {
            Phase::Correction => {
                // Day 1: up-close bar where the trailing low_lookback-bar low
                // was set within the last low_recency bars.
                let win_start = i.saturating_sub(config.low_lookback);
                let window = &bars[win_start..i];
                let min_low = window.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
                let recency_start = i.saturating_sub(config.low_recency);
                let recent_min = bars[recency_start..i]
                    .iter()
                    .map(|b| b.low)
                    .fold(f64::INFINITY, f64::min);
                if bar.close > prev.close && recent_min <= min_low {
                    phase = Phase::AttemptedRally;
                    rally_day = 1;
                    day1_low = recent_min.min(bar.low);
                    details.rally_day = Some(1);
                    details.day1_date = Some(bar_date(bar.timestamp_unix_ms));
                    details.day1_low = Some(day1_low);
                }
            }
            Phase::AttemptedRally => {
                rally_day += 1;
                details.rally_day = Some(rally_day);
                if bar.low < day1_low {
                    phase = Phase::Correction;
                    details.notes.push(format!(
                        "rally attempt failed on {}: undercut Day-1 low {:.2}",
                        bar_date(bar.timestamp_unix_ms),
                        day1_low
                    ));
                    details.rally_day = None;
                    details.day1_date = None;
                    details.day1_low = None;
                } else if rally_day >= config.ft_window_start
                    && rally_day <= config.ft_window_end
                    && prev.volume > 0.0
                    && bar.close / prev.close - 1.0 >= config.min_gain_pct
                    && bar.volume > prev.volume
                {
                    phase = Phase::ConfirmedUptrend;
                    let gain = bar.close / prev.close - 1.0;
                    details.follow_through_date = Some(bar_date(bar.timestamp_unix_ms));
                    details.follow_through_gain_pct = Some(gain);
                    details.follow_through_volume_ratio = Some(bar.volume / prev.volume);
                    details.follow_through_low = Some(bar.low);
                    details.rally_day = None;
                } else if rally_day > config.ft_window_end {
                    phase = Phase::Correction;
                    details.notes.push(format!(
                        "rally attempt stalled: no follow-through by day {} (as of {})",
                        config.ft_window_end,
                        bar_date(bar.timestamp_unix_ms)
                    ));
                    details.rally_day = None;
                    details.day1_date = None;
                    details.day1_low = None;
                }
            }
            Phase::ConfirmedUptrend => {
                // v1 invalidation: undercut the follow-through bar's low.
                if let Some(ft_low) = details.follow_through_low
                    && bar.low < ft_low
                {
                    phase = Phase::Correction;
                    details.notes.push(format!(
                        "confirmed uptrend invalidated on {}: undercut follow-through low {:.2}",
                        bar_date(bar.timestamp_unix_ms),
                        ft_low
                    ));
                    details.follow_through_date = None;
                    details.follow_through_gain_pct = None;
                    details.follow_through_volume_ratio = None;
                    details.follow_through_low = None;
                }
            }
        }
    }

    let state = match phase {
        Phase::Correction => RegimeState::Correction,
        Phase::AttemptedRally => RegimeState::AttemptedRally,
        Phase::ConfirmedUptrend => RegimeState::ConfirmedUptrend,
    };

    Ok(FtdResult {
        detector: "FTD".to_string(),
        state,
        as_of,
        symbol,
        details,
    })
}

/// Format a millisecond Unix timestamp as YYYY-MM-DD (UTC).
fn bar_date(timestamp_unix_ms: i64) -> String {
    DateTime::from_timestamp_millis(timestamp_unix_ms)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    /// Build `n` daily bars starting 2026-01-05 (a Monday), ascending.
    fn bars(closes: &[f64], volumes: &[f64]) -> BarSeries {
        assert_eq!(closes.len(), volumes.len());
        // 2026-01-05T00:00:00Z in millis
        let t0: i64 = 1_767_571_200_000; // 2026-01-05T00:00:00Z
        let day: i64 = 86_400_000;
        let bars = closes
            .iter()
            .zip(volumes.iter())
            .enumerate()
            .map(|(i, (&c, &v))| {
                let o = c * 0.999;
                Bar {
                    symbol: "SPY".to_string(),
                    market: "equities".to_string(),
                    timeframe: "1d".to_string(),
                    timestamp_unix_ms: t0 + (i as i64) * day,
                    open: o,
                    high: c * 1.002,
                    low: c * 0.998,
                    close: c,
                    volume: v,
                    adjusted_close: None,
                }
            })
            .collect();
        BarSeries {
            schema_version: "v0".to_string(),
            bars,
        }
    }

    /// Decline into a fresh low, Day 1 up-close, then a follow-through on
    /// rally day 5 (+1.5% on rising volume).
    fn confirmation_series() -> BarSeries {
        // 25 down-drift bars, then Day 1, then grind, then FTD.
        let mut closes: Vec<f64> = (0..25).map(|i| 500.0 - i as f64 * 2.0).collect();
        let day1 = *closes.last().unwrap() * 1.004; // up close after fresh low
        closes.push(day1);
        closes.push(day1 * 1.002);
        closes.push(day1 * 1.001);
        closes.push(day1 * 1.003);
        let ftd = day1 * 1.003 * 1.015; // +1.5% on rally day 5
        closes.push(ftd);
        closes.push(ftd * 1.001);
        let volumes: Vec<f64> = closes
            .iter()
            .enumerate()
            .map(|(i, _)| if i == 29 { 2_000_000.0 } else { 1_000_000.0 })
            .collect();
        bars(&closes, &volumes)
    }

    #[test]
    fn confirms_uptrend_on_follow_through_day() {
        let r = detect(&confirmation_series(), &FtdConfig::default()).unwrap();
        assert_eq!(r.state, RegimeState::ConfirmedUptrend);
        assert_eq!(r.detector, "FTD");
        assert_eq!(r.as_of, "2026-02-04");
        let d = &r.details;
        assert!(d.follow_through_date.is_some());
        assert!(d.follow_through_gain_pct.unwrap() >= 0.01);
        assert!(d.follow_through_volume_ratio.unwrap() > 1.0);
        assert!(d.follow_through_low.is_some());
    }

    #[test]
    fn undercut_returns_to_correction() {
        let mut closes: Vec<f64> = (0..25).map(|i| 500.0 - i as f64 * 2.0).collect();
        let day1 = *closes.last().unwrap() * 1.004;
        closes.push(day1); // Day 1
        // Next bar undercuts the Day-1 low (low = close*0.998 < day1_low).
        closes.push(day1 * 0.985);
        closes.push(day1 * 0.98);
        let n = closes.len();
        let volumes = vec![1_000_000.0; n];
        let r = detect(&bars(&closes, &volumes), &FtdConfig::default()).unwrap();
        assert_eq!(r.state, RegimeState::Correction);
        assert!(r.details.notes.iter().any(|n| n.contains("undercut")));
    }

    #[test]
    fn follow_through_too_early_does_not_confirm() {
        // FTD-strength bar on rally day 2 (outside 3..=10 window), then
        // nothing qualifies through day 10 -> stalled back to Correction.
        let mut closes: Vec<f64> = (0..25).map(|i| 500.0 - i as f64 * 2.0).collect();
        let day1 = *closes.last().unwrap() * 1.004;
        closes.push(day1); // rally day 1
        closes.push(day1 * 1.02); // rally day 2: +2% on huge volume — too early
        for _ in 0..9 {
            let last = *closes.last().unwrap();
            closes.push(last * 1.001); // drift, never +1%
        }
        let mut volumes = vec![1_000_000.0; closes.len()];
        volumes[26] = 3_000_000.0;
        let r = detect(&bars(&closes, &volumes), &FtdConfig::default()).unwrap();
        assert_eq!(r.state, RegimeState::Correction);
        assert!(r.details.notes.iter().any(|n| n.contains("stalled")));
        assert!(r.details.follow_through_date.is_none());
    }

    #[test]
    fn confirmed_uptrend_invalidated_on_ftd_low_undercut() {
        let mut s = confirmation_series();
        // Append a bar that undercuts the follow-through bar's low.
        let last = s.bars.last().unwrap().clone();
        let mut crash = last.clone();
        crash.timestamp_unix_ms += 86_400_000;
        crash.open = last.close * 0.99;
        crash.high = last.close * 0.995;
        crash.low = last.close * 0.97; // well under the FTD bar's low
        crash.close = last.close * 0.98;
        crash.volume = 2_500_000.0;
        s.bars.push(crash);
        let r = detect(&s, &FtdConfig::default()).unwrap();
        assert_eq!(r.state, RegimeState::Correction);
        assert!(r.details.notes.iter().any(|n| n.contains("invalidated")));
    }

    #[test]
    fn empty_input_is_unknown_not_error() {
        let s = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![],
        };
        let r = detect(&s, &FtdConfig::default()).unwrap();
        assert_eq!(r.state, RegimeState::Unknown);
        assert!(!r.details.notes.is_empty());
    }

    #[test]
    fn bad_input_fails_closed() {
        let mut s = confirmation_series();
        s.bars[10].close = f64::NAN;
        let err = detect(&s, &FtdConfig::default()).unwrap_err();
        assert!(matches!(err, RegimeError::BadInput(10, _)));
    }
}
