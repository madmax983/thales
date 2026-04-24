#![cfg(feature = "nova")]

//! Market Aerodynamics Module
//!
//! This module analyzes the "aerodynamic drag" of a market. It measures how much
//! volume is required to move the price by a certain percentage.
//!
//! - **High Drag**: The market requires massive volume to move the price (e.g., highly liquid, thick order books).
//! - **Low Drag**: The price moves easily with little volume (e.g., illiquid, highly volatile).
//!
//! This helps in understanding market resistance and potential for explosive moves.

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// The output report containing the calculated market aerodynamics.
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::market_aerodynamics::AerodynamicsReport;
///
/// let report = AerodynamicsReport {
///     drag_coefficient: 1.5,
///     aerodynamic_status: "High Drag".to_string(),
/// };
///
/// assert_eq!(report.drag_coefficient, 1.5);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AerodynamicsReport {
    /// The calculated drag coefficient. (Volume / Price Change %)
    pub drag_coefficient: f64,
    /// The status of the market's aerodynamics.
    pub aerodynamic_status: String,
}

/// Calculates the drag coefficient for the market.
///
/// The drag coefficient is defined as the average volume required to move the price by 1%.
///
/// # Arguments
/// * `series` - The historical [`BarSeries`] data to analyze.
///
/// # Examples
/// ```rust
/// #[cfg(feature = "nova")]
/// # {
/// use contracts::{Bar, BarSeries};
/// use thales_cli::experimental::market_aerodynamics::analyze_aerodynamics;
///
/// let series = BarSeries {
///     schema_version: "v0".to_string(),
///     bars: vec![Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 0, open: 100.0, high: 105.0, low: 100.0, close: 105.0, volume: 1000.0 }],
/// };
///
/// let report_opt = analyze_aerodynamics(&series);
/// assert!(report_opt.is_some());
/// if let Some(report) = report_opt {
///     // Price moved from 100 to 105 (5%). Volume = 1000.
///     // Drag = 1000 / 5 = 200.
///     assert_eq!(report.drag_coefficient, 200.0);
/// }
/// # }
/// ```
pub fn analyze_aerodynamics(series: &BarSeries) -> Option<AerodynamicsReport> {
    if series.bars.is_empty() {
        return None;
    }

    let mut total_drag = 0.0;
    let mut valid_bars = 0;

    for bar in &series.bars {
        let price_change_pct = if bar.open > 0.0 {
            ((bar.close - bar.open).abs() / bar.open) * 100.0
        } else {
            0.0
        };

        if price_change_pct > 0.0 {
            total_drag += bar.volume / price_change_pct;
            valid_bars += 1;
        }
    }

    if valid_bars == 0 {
        return None;
    }

    let drag_coefficient = total_drag / valid_bars as f64;

    let aerodynamic_status = if drag_coefficient > 5000.0 {
        "High Drag".to_string()
    } else if drag_coefficient > 1000.0 {
        "Medium Drag".to_string()
    } else {
        "Low Drag".to_string()
    };

    Some(AerodynamicsReport {
        drag_coefficient,
        aerodynamic_status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_analyze_aerodynamics() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![
                Bar {
                    symbol: "TEST".into(),
                    market: "test".into(),
                    timeframe: "1d".into(),
                    timestamp_unix_ms: 0,
                    open: 100.0,
                    high: 105.0,
                    low: 100.0,
                    close: 105.0,
                    volume: 1000.0,
                }, // Drag = 1000 / 5 = 200
                Bar {
                    symbol: "TEST".into(),
                    market: "test".into(),
                    timeframe: "1d".into(),
                    timestamp_unix_ms: 1,
                    open: 105.0,
                    high: 110.0,
                    low: 100.0,
                    close: 100.0,
                    volume: 2000.0,
                }, // Drag = 2000 / (5/105 * 100) = 2000 / 4.7619 = 420
            ],
        };

        let report_opt = analyze_aerodynamics(&series);
        assert!(report_opt.is_some());
        let report = report_opt.unwrap();
        // 200 + 420 = 620 / 2 = 310
        assert_eq!(report.drag_coefficient.round(), 310.0);
        assert_eq!(report.aerodynamic_status, "Low Drag");
    }

    #[test]
    fn test_analyze_aerodynamics_no_price_change() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![Bar {
                symbol: "TEST".into(),
                market: "test".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 0,
                open: 100.0,
                high: 100.0,
                low: 100.0,
                close: 100.0,
                volume: 1000.0,
            }],
        };

        let report_opt = analyze_aerodynamics(&series);
        assert!(report_opt.is_none());
    }
}
