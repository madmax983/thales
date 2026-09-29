//! Regime change detectors for Thales.
//!
//! This crate answers **"has the market regime changed?"** — it never answers
//! "what should I trade?". Its outputs are [`RegimeState`] readings, never
//! [`TradeIntent`](contracts::TradeIntent)s. Nothing here can clear the Jev
//! gate, weaken a threshold, or reach execution. Regime context is advisory
//! evidence: it feeds `Market_Regime.md` and `analyze-market --research`, not
//! signal generation.
//!
//! # Detectors
//!
//! - [`ftd`]: Follow-Through Day state machine (O'Neil CAN SLIM), computed
//!   deterministically from daily index bars + volume. The independent,
//!   paywall-free fallback.
//! - [`zbt`]: curated historical Zweig Breadth Thrust dates from published
//!   technical sources. Reported events are the authority for rare thrusts —
//!   there is no free NYSE breadth feed, and this crate will not fake one.
//! - [`events`]: append-only reported-regime-events log
//!   (`docs/regime-events.jsonl`), maintained by the scan's research step.
//! - [`merge`]: merges the computed FTD reading with reported events and ZBT
//!   history into one [`RegimeReport`] — the unit the coordinator consumes.
//!
//! All detectors are pure functions of their inputs: no randomness, no I/O
//! (except [`events::read_events_log`], which only reads), no hidden state.
//! Missing or empty inputs yield [`RegimeState::Unknown`] with a note — empty
//! is a result, not a failure.

pub mod events;
pub mod ftd;
pub mod merge;
pub mod zbt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from regime detection. Every failure mode is typed so callers (and
/// the CLI) fail closed instead of silently returning a nonsense reading.
#[derive(Debug, Error, PartialEq)]
pub enum RegimeError {
    /// A price, volume, or date value was NaN, infinite, non-positive, or
    /// otherwise unusable where a real value was required.
    #[error("invalid input at position {0}: {1}")]
    BadInput(usize, String),
    /// The events log file could not be read or a line failed to parse.
    #[error("events log error: {0}")]
    EventsLog(String),
    /// The embedded ZBT history failed to parse (a build-time data bug).
    #[error("ZBT history data error: {0}")]
    ZbtHistory(String),
}

/// The market-regime reading. Three real states plus [`RegimeState::Unknown`]
/// for "the inputs did not support a reading".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegimeState {
    /// No confirmed uptrend: the market is correcting or a rally attempt
    /// failed/stalled.
    Correction,
    /// A rally attempt is underway (Day 1 printed) but no follow-through day
    /// has confirmed it yet.
    AttemptedRally,
    /// A follow-through day confirmed the rally attempt.
    ConfirmedUptrend,
    /// Inputs were missing, empty, or unusable — no reading. A result, not a
    /// failure.
    Unknown,
}

impl RegimeState {
    /// Canonical string form used in JSON output.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            RegimeState::Correction => "Correction",
            RegimeState::AttemptedRally => "AttemptedRally",
            RegimeState::ConfirmedUptrend => "ConfirmedUptrend",
            RegimeState::Unknown => "Unknown",
        }
    }
}

/// Which evidence backs the merged regime reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetectorBasis {
    /// Only reported events (IBD market status, published ZBT/FTD calls).
    Reported,
    /// Only the computed FTD state machine.
    Computed,
    /// Computed and reported agree.
    Both,
}

impl DetectorBasis {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            DetectorBasis::Reported => "reported",
            DetectorBasis::Computed => "computed",
            DetectorBasis::Both => "both",
        }
    }
}
