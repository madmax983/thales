#![cfg(feature = "nova")]

//! Time Dilation Module
//!
//! This module analyzes the "dilation" of trading volume over time relative to a baseline.
//! Instead of viewing market volume statically (e.g. comparing daily volume to moving averages),
//! it measures the rate at which volume aggregates over time.
//!
//! Volume is a crucial metric, acting as the fuel behind significant market movements.
//! A sudden surge in volume over a compressed time span points to a "dilated" event where
//! time feels faster—often an indicator of market panic or euphoria prior to major breakouts
//! or reversals.
//!
//! By returning a calculated `dilation_factor`, this module provides traders with a tangible
//! measure of volume acceleration or deceleration.

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// The output report containing the calculated time dilation factor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeDilationReport {
    /// The average volume aggregated per time step across the whole series.
    pub base_volume_rate: f64,
    /// The ratio between the recent volume accumulation rate and the base historical rate.
    pub dilation_factor: f64,
}

/// Analyzes a [`BarSeries`] to calculate its Time Dilation factor.
///
/// This function computes the base volume aggregation rate over the entire historical window,
/// and then calculates the recent accumulation rate using the latest 30% of the dataset.
/// A returned `dilation_factor` greater than 1.0 indicates that volume is accumulating
/// more rapidly than the historical baseline.
///
/// # Arguments
/// * `series` - The historical [`BarSeries`] data containing volume metrics.
///
/// # Examples
/// ```rust
/// #[cfg(feature = "nova")]
/// # {
/// use contracts::{Bar, BarSeries};
/// use thales_cli::experimental::time_dilation::analyze_time_dilation;
///
/// let series = BarSeries {
///     schema_version: "v0".to_string(),
///     bars: vec![
///         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 1000, open: 10.0, high: 20.0, low: 5.0, close: 15.0, volume: 100.0 },
///         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 2000, open: 15.0, high: 25.0, low: 10.0, close: 20.0, volume: 100.0 },
///         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 3000, open: 20.0, high: 30.0, low: 15.0, close: 25.0, volume: 400.0 },
///     ],
/// };
///
/// let report_opt = analyze_time_dilation(&series);
/// assert!(report_opt.is_some());
/// if let Some(report) = report_opt {
///     // Base rate is (100 + 100 + 400) / 3 = 200
///     // The latest bar serves as the recent 30% rate window. 400 / 1 = 400.
///     // The dilation factor is 400 / 200 = 2.0
///     assert_eq!(report.base_volume_rate, 200.0);
///     assert_eq!(report.dilation_factor, 2.0);
/// }
/// # }
/// ```
pub fn analyze_time_dilation(series: &BarSeries) -> Option<TimeDilationReport> {
    if series.bars.is_empty() {
        return None;
    }

    let total_volume: f64 = series.bars.iter().map(|b| b.volume).sum();
    let n = series.bars.len();

    if n == 0 || total_volume == 0.0 {
        return None;
    }

    let base_volume_rate = total_volume / (n as f64);

    // We consider "recent" time as the last ~30% of the series (at least 1 bar)
    let recent_len = (n as f64 * 0.3).ceil() as usize;
    let recent_len = recent_len.max(1);

    let recent_bars = &series.bars[n - recent_len..];
    let recent_volume: f64 = recent_bars.iter().map(|b| b.volume).sum();

    let recent_rate = recent_volume / (recent_len as f64);

    let dilation_factor = recent_rate / base_volume_rate;

    Some(TimeDilationReport {
        base_volume_rate,
        dilation_factor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_time_dilation_calculates_properly() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![
                Bar {
                    symbol: "TEST".into(),
                    market: "test".into(),
                    timeframe: "1d".into(),
                    timestamp_unix_ms: 1000,
                    open: 10.0,
                    high: 20.0,
                    low: 5.0,
                    close: 15.0,
                    volume: 100.0,
                },
                Bar {
                    symbol: "TEST".into(),
                    market: "test".into(),
                    timeframe: "1d".into(),
                    timestamp_unix_ms: 2000,
                    open: 15.0,
                    high: 25.0,
                    low: 10.0,
                    close: 20.0,
                    volume: 100.0,
                },
                Bar {
                    symbol: "TEST".into(),
                    market: "test".into(),
                    timeframe: "1d".into(),
                    timestamp_unix_ms: 3000,
                    open: 20.0,
                    high: 30.0,
                    low: 15.0,
                    close: 25.0,
                    volume: 400.0,
                },
            ],
        };

        let report = analyze_time_dilation(&series).expect("Should return a report");
        assert_eq!(report.base_volume_rate, 200.0);
        // The last bar is 1/3 (since ceil(3 * 0.3) = 1)
        // Base rate = 600 / 3 = 200.
        // Recent rate = 400 / 1 = 400.
        // Dilation factor = 400 / 200 = 2.0.
        assert_eq!(report.dilation_factor, 2.0);
    }
}
