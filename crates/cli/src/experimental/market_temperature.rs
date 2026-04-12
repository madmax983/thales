#![cfg(feature = "nova")]

//! Market Temperature Module
//!
//! This module analyzes the "heat" of the market by combining volume and the rate of change
//! into a single metric. While traditional analysis looks at volatility (seismology) or
//! directional pull (gravity), Market Temperature attempts to quantify the raw energy being
//! expended.
//!
//! A high temperature indicates an overheating market—often seen right before a blow-off top—
//! while a low temperature indicates a freezing, low-energy market that might precede an
//! explosive move.
//!
//! By calculating a volume-weighted absolute rate of return, traders can gauge whether
//! current price action is backed by significant conviction (heat) or just drifting (cold).

use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Configuration for calculating the Market Temperature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureConfig {
    /// The number of recent periods to include in the temperature calculation.
    pub window_size: usize,
}

/// The output report containing the calculated market temperature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureReport {
    /// The calculated temperature metric. Higher values indicate a "hotter" market.
    pub temperature: f64,
}

/// Calculates the Market Temperature over a given window.
///
/// This function measures the raw "heat" of the market by calculating the volume-weighted
/// absolute rate of change. It is useful for detecting potential overheating conditions
/// before a trend reversal, or freezing conditions before a breakout.
///
/// # Arguments
/// * `series` - The historical [`BarSeries`] data to analyze.
/// * `config` - The [`TemperatureConfig`] specifying the lookback window.
///
/// # Examples
/// ```rust
/// #[cfg(feature = "nova")]
/// # {
/// use contracts::{Bar, BarSeries};
/// use thales_cli::experimental::market_temperature::{analyze_temperature, TemperatureConfig};
///
/// let series = BarSeries {
///     schema_version: "v1".to_string(),
///     bars: vec![
///         Bar { symbol: "BTC".into(), market: "crypto".into(), timeframe: "1d".into(), timestamp_unix_ms: 0, open: 100.0, high: 110.0, low: 90.0, close: 100.0, volume: 1000.0 },
///         Bar { symbol: "BTC".into(), market: "crypto".into(), timeframe: "1d".into(), timestamp_unix_ms: 1, open: 100.0, high: 120.0, low: 100.0, close: 110.0, volume: 2000.0 },
///     ],
/// };
///
/// let config = TemperatureConfig { window_size: 2 };
/// let report = analyze_temperature(&series, config).unwrap();
///
/// // The temperature is scaled by 10000 for readability.
/// assert!(report.temperature > 0.0);
/// # }
/// ```
pub fn analyze_temperature(
    series: &BarSeries,
    config: TemperatureConfig,
) -> Result<TemperatureReport> {
    if series.bars.len() < config.window_size || config.window_size == 0 {
        return Ok(TemperatureReport { temperature: 0.0 });
    }

    let start_idx = series.bars.len() - config.window_size;
    let window = &series.bars[start_idx..];

    let mut total_volume = 0.0;
    let mut weighted_roc = 0.0;

    for i in 1..window.len() {
        let prev_close = window[i - 1].close;
        let current_close = window[i].close;
        let volume = window[i].volume;

        if prev_close > 0.0 {
            let roc = (current_close - prev_close) / prev_close;
            weighted_roc += roc.abs() * volume;
        }
        total_volume += volume;
    }

    let temperature = if total_volume > 0.0 {
        (weighted_roc / total_volume) * 10000.0 // Scaled for readability
    } else {
        0.0
    };

    Ok(TemperatureReport { temperature })
}

/// Prints a visual ASCII representation of the market temperature.
pub fn print_ascii_temperature(report: &TemperatureReport) {
    println!("Market Temperature Report");
    println!("-------------------------");
    println!("Temperature: {:.2}°", report.temperature);
    let bar_len = (report.temperature / 10.0).min(50.0) as usize;
    let bar: String = "🔥".repeat(bar_len.max(1));
    println!("Heat Index:  [{}]", bar);
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_analyze_temperature() {
        let series = BarSeries {
            schema_version: "v1".to_string(),
            bars: vec![
                Bar {
                    symbol: "BTC".to_string(),
                    market: "crypto".to_string(),
                    timeframe: "1d".to_string(),
                    timestamp_unix_ms: 0,
                    open: 100.0,
                    high: 110.0,
                    low: 90.0,
                    close: 105.0,
                    volume: 1000.0,
                },
                Bar {
                    symbol: "BTC".to_string(),
                    market: "crypto".to_string(),
                    timeframe: "1d".to_string(),
                    timestamp_unix_ms: 1,
                    open: 105.0,
                    high: 120.0,
                    low: 100.0,
                    close: 115.0,
                    volume: 2000.0,
                },
            ],
        };
        let config = TemperatureConfig { window_size: 2 };
        let result = analyze_temperature(&series, config);
        assert!(result.is_ok());
    }
}
