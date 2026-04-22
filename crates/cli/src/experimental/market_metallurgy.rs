#![cfg(feature = "nova")]

//! Market Metallurgy Module
//!
//! This module analyzes market data by mapping price action to metallurgical concepts.
//! Just as metals are shaped by heat and pressure, markets are shaped by volume and volatility.
//! We measure 'tensile strength' (how much volume is needed to move price), 'malleability'
//! (ability to stretch in price range), and 'heat treatment' (accumulation of volatility).
//!
//! # Examples
//! ```rust
//! #[cfg(feature = "nova")]
//! # {
//! use thales_cli::experimental::market_metallurgy::analyze_metallurgy;
//! use contracts::{BarSeries, Bar};
//!
//! let series = BarSeries {
//!     schema_version: "v0".to_string(),
//!     bars: vec![
//!         Bar { symbol: "TEST".to_string(), market: "test".to_string(), timeframe: "1d".to_string(), timestamp_unix_ms: 0, open: 100.0, high: 110.0, low: 90.0, close: 105.0, volume: 1000.0 },
//!         Bar { symbol: "TEST".to_string(), market: "test".to_string(), timeframe: "1d".to_string(), timestamp_unix_ms: 1000, open: 105.0, high: 115.0, low: 95.0, close: 110.0, volume: 2000.0 },
//!     ],
//! };
//!
//! if let Some(report) = analyze_metallurgy(&series) {
//!     println!("Malleability: {}", report.malleability);
//! }
//! # }
//! ```

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetallurgyReport {
    pub tensile_strength: f64,
    pub malleability: f64,
    pub heat_treatment: f64,
}

pub fn analyze_metallurgy(series: &BarSeries) -> Option<MetallurgyReport> {
    if series.bars.is_empty() {
        return None;
    }

    let mut max_high = f64::MIN;
    let mut min_low = f64::MAX;
    let mut total_volume = 0.0;

    let mut prev_close = series.bars.first()?.close;
    let mut total_heat = 0.0; // measure of consecutive up/down swings

    for bar in &series.bars {
        if bar.high > max_high {
            max_high = bar.high;
        }
        if bar.low < min_low {
            min_low = bar.low;
        }
        total_volume += bar.volume;

        let swing = (bar.close - prev_close).abs();
        total_heat += swing;
        prev_close = bar.close;
    }

    let price_range = max_high - min_low;
    let avg_volume = total_volume / series.bars.len() as f64;

    // Tensile Strength: How much volume does it take to move the price? (Resistance to breaking)
    let tensile_strength = if price_range > 0.0 {
        avg_volume / price_range
    } else {
        0.0
    };

    // Malleability: How wide is the price range relative to the starting price? (Ability to stretch)
    let base_price = series.bars.first()?.close;
    let malleability = if base_price > 0.0 {
        price_range / base_price
    } else {
        0.0
    };

    // Heat Treatment: Normalized accumulation of volatility (swings) over time
    let heat_treatment = if price_range > 0.0 {
        total_heat / price_range
    } else {
        0.0
    };

    Some(MetallurgyReport {
        tensile_strength,
        malleability,
        heat_treatment,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_analyze_metallurgy() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![
                Bar {
                    symbol: "GOLD".to_string(),
                    market: "commodities".to_string(),
                    timeframe: "1d".to_string(),
                    timestamp_unix_ms: 0,
                    open: 100.0,
                    high: 105.0,
                    low: 95.0,
                    close: 102.0,
                    volume: 1000.0,
                },
                Bar {
                    symbol: "GOLD".to_string(),
                    market: "commodities".to_string(),
                    timeframe: "1d".to_string(),
                    timestamp_unix_ms: 86400000,
                    open: 102.0,
                    high: 110.0,
                    low: 100.0,
                    close: 108.0,
                    volume: 1200.0,
                },
            ],
        };

        let report = analyze_metallurgy(&series).unwrap();
        assert!(report.malleability > 0.0);
        assert!(report.tensile_strength > 0.0);
        assert!(report.heat_treatment > 0.0);
    }
}
