#![cfg(feature = "nova")]

//! Market Resonance Module
//!
//! This module measures market resonance by analyzing the combined variance
//! of price and volume over a given window. High resonance indicates a market
//! entering a high-energy, potentially explosive state.
//!
//! # Examples
//! ```rust
//! #[cfg(feature = "nova")]
//! # {
//! use thales_cli::experimental::resonance::{analyze_resonance, ResonanceConfig};
//! use contracts::{BarSeries, Bar};
//!
//! let mut bars = Vec::new();
//! for i in 0..10 {
//!     bars.push(Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: i * 1000, open: 100.0, high: 105.0, low: 95.0, close: 100.0, volume: 1000.0 });
//! }
//! let series = BarSeries { schema_version: "v0".to_string(), bars };
//! let config = ResonanceConfig { window_size: 5, amplitude_threshold: 10.0 };
//!
//! if let Ok(report) = analyze_resonance(&series, config) {
//!     println!("Combined Resonance: {}", report.combined_resonance);
//! }
//! # }
//! ```


use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResonanceConfig {
    pub window_size: usize,
    pub amplitude_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResonanceReport {
    pub price_resonance: f64,
    pub volume_resonance: f64,
    pub combined_resonance: f64,
    pub is_resonating: bool,
}

pub fn analyze_resonance(
    series: &BarSeries,
    config: ResonanceConfig,
) -> anyhow::Result<ResonanceReport> {
    if series.bars.len() < config.window_size || config.window_size < 2 {
        return Ok(ResonanceReport {
            price_resonance: 0.0,
            volume_resonance: 0.0,
            combined_resonance: 0.0,
            is_resonating: false,
        });
    }

    let n = series.bars.len();
    let window = &series.bars[(n - config.window_size)..n];

    let mut price_sum = 0.0;
    let mut volume_sum = 0.0;
    for bar in window {
        price_sum += bar.close;
        volume_sum += bar.volume;
    }
    let price_mean = price_sum / config.window_size as f64;
    let volume_mean = volume_sum / config.window_size as f64;

    let mut price_var_sum = 0.0;
    let mut volume_var_sum = 0.0;
    for bar in window {
        price_var_sum += (bar.close - price_mean).powi(2);
        volume_var_sum += (bar.volume - volume_mean).powi(2);
    }

    // Simple variance as a proxy for "resonance/energy"
    let price_resonance = price_var_sum / config.window_size as f64;
    let volume_resonance = volume_var_sum / config.window_size as f64;

    // Combine them (scaled for easier reading if needed, but let's keep it simple)
    // We'll normalize volume resonance by a factor of 1000.0 to bring it closer in scale, just an arbitrary choice
    let normalized_volume_res = volume_resonance / 1000.0;
    let combined_resonance = price_resonance * normalized_volume_res;

    let is_resonating = combined_resonance > config.amplitude_threshold;

    Ok(ResonanceReport {
        price_resonance,
        volume_resonance,
        combined_resonance,
        is_resonating,
    })
}

pub fn print_ascii_resonance(report: &ResonanceReport) {
    println!("=== Market Resonance Analysis ===");
    println!("Price Resonance:    {:.4}", report.price_resonance);
    println!("Volume Resonance:   {:.4}", report.volume_resonance);
    println!("Combined Resonance: {:.4}", report.combined_resonance);
    if report.is_resonating {
        println!("Status:             🚀 HIGH RESONANCE DETECTED");
    } else {
        println!("Status:             Normal / Low Energy");
    }
    println!("=================================");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_mock_bar(close: f64, volume: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "test".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 0,
            open: close,
            high: close,
            low: close,
            close,
            volume,
        }
    }

    #[test]
    fn test_analyze_resonance() {
        let bars = vec![
            create_mock_bar(100.0, 1000.0),
            create_mock_bar(110.0, 1500.0),
            create_mock_bar(105.0, 1200.0),
            create_mock_bar(120.0, 2000.0),
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = ResonanceConfig {
            window_size: 4,
            amplitude_threshold: 100.0, // Arbitrary threshold for testing
        };

        let report = analyze_resonance(&series, config).unwrap();

        // Mean price = (100+110+105+120)/4 = 108.75
        // Price Variance = ((100-108.75)^2 + (110-108.75)^2 + (105-108.75)^2 + (120-108.75)^2)/4
        // = (76.5625 + 1.5625 + 14.0625 + 126.5625) / 4 = 218.75 / 4 = 54.6875

        // Mean volume = (1000+1500+1200+2000)/4 = 1425
        // Volume Variance = ((1000-1425)^2 + (1500-1425)^2 + (1200-1425)^2 + (2000-1425)^2)/4
        // = (180625 + 5625 + 50625 + 330625)/4 = 567500 / 4 = 141875

        // Combined = 54.6875 * (141875 / 1000) = 54.6875 * 141.875 = 7758.7890625

        assert!(report.price_resonance > 0.0);
        assert!(report.volume_resonance > 0.0);
        assert!(report.combined_resonance > 0.0);
        assert!(report.is_resonating); // 7758.78 > 100
    }
}
