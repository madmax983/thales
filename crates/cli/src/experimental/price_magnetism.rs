#![cfg(feature = "nova")]

//! Price Magnetism Module
//!
//! This module analyzes the "magnetic pull" of significant price levels, such as psychological
//! round numbers. Markets often naturally gravitate towards or repel from certain major price
//! thresholds due to human psychology, institutional orders, and option strikes.
//!
//! While technical indicators often measure momentum or trend strength, Price Magnetism
//! attempts to quantify the immediate force pulling price back to a nearby psychological anchor.
//!
//! By calculating the distance and direction to the nearest "round number", traders can
//! gauge the `net_magnetic_force` and anticipate short-term reversions or breakouts.

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// The output report containing the calculated price magnetism.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceMagnetismReport {
    /// The most recent close price of the asset.
    pub current_price: f64,
    /// The calculated magnetic force. A positive value indicates an upward pull towards
    /// a higher round number, while a negative value indicates a downward pull.
    pub net_magnetic_force: f64,
}

/// Calculates the net magnetic force of the current price relative to a psychological round number.
///
/// This simplistic model rounds the price to the nearest multiple of 10.0 and calculates
/// the force as the difference between the target round number and the current price.
///
/// # Arguments
/// * `series` - The historical [`BarSeries`] data to analyze.
///
/// # Examples
/// ```rust
/// #[cfg(feature = "nova")]
/// # {
/// use contracts::{Bar, BarSeries};
/// use thales_cli::experimental::price_magnetism::analyze_magnetism;
///
/// let series = BarSeries {
///     schema_version: "v0".to_string(),
///     bars: vec![Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 0, open: 10.0, high: 20.0, low: 5.0, close: 100.5, volume: 100.0 }],
/// };
///
/// let report_opt = analyze_magnetism(&series);
/// assert!(report_opt.is_some());
/// if let Some(report) = report_opt {
///     assert_eq!(report.current_price, 100.5);
///     // The nearest multiple of 10 is 100.0. The net force is 100.0 - 100.5 = -0.5.
///     assert!(report.net_magnetic_force < 0.0);
/// }
/// # }
/// ```
pub fn analyze_magnetism(series: &BarSeries) -> Option<PriceMagnetismReport> {
    if series.bars.is_empty() {
        return None;
    }

    let current_price = series.bars.last()?.close;

    // Simplistic gravity/magnetism algorithm to nearby round numbers:
    // E.g. if price is 100.5, the nearest round number might be 100.0
    // We consider round numbers to be multiples of 10.0 for this simple prototype
    let nearest_round = (current_price / 10.0).round() * 10.0;

    // Magnetic force is proportional to distance but pulls towards the round number
    // Positive means upward pull, negative means downward pull
    let net_magnetic_force = nearest_round - current_price;

    Some(PriceMagnetismReport {
        current_price,
        net_magnetic_force,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_analyze_magnetism_calculates_correct_force() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![Bar {
                symbol: "TEST".into(),
                market: "test".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 0,
                open: 10.0,
                high: 20.0,
                low: 5.0,
                close: 100.5,
                volume: 100.0,
            }],
        };

        let report_opt = analyze_magnetism(&series);
        assert!(report_opt.is_some());
        if let Some(report) = report_opt {
            assert_eq!(report.current_price, 100.5);
            assert!(report.net_magnetic_force < 0.0);
        }
    }
}
