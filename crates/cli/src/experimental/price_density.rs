//! Price Density Module
//!
//! This module calculates the price density, which measures the amount
//! of time spent at each price level. It serves as a primitive alternative
//! to volume profile, focusing purely on price action to identify zones
//! of heavy consolidation or swift rejection.
//!
//! # Examples
//! ```rust
//! use thales_cli::experimental::price_density::calculate_price_density;
//! use contracts::{BarSeries, Bar};
//!
//! let series = BarSeries {
//!     schema_version: "v0".to_string(),
//!     bars: vec![
//!         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 0, open: 100.0, high: 110.0, low: 90.0, close: 105.0, volume: 1000.0 },
//!         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 1000, open: 105.0, high: 115.0, low: 95.0, close: 110.0, volume: 2000.0 },
//!     ],
//! };
//!
//! if let Ok(density) = calculate_price_density(&series, 5) {
//!     println!("Density bins: {}", density.len());
//! }
//! ```

use anyhow::Result;
use contracts::BarSeries;
use std::collections::HashMap;

/// Calculates the price density (amount of time spent at each price level).
///
/// This provides a primitive alternative to volume profile, purely based on price action.
pub fn calculate_price_density(series: &BarSeries, bins: usize) -> Result<HashMap<String, usize>> {
    let mut min_price = f64::MAX;
    let mut max_price = f64::MIN;

    for bar in &series.bars {
        if bar.low < min_price {
            min_price = bar.low;
        }
        if bar.high > max_price {
            max_price = bar.high;
        }
    }

    if min_price == f64::MAX || max_price == f64::MIN || bins == 0 || min_price == max_price {
        return Ok(HashMap::new());
    }

    let bin_size = (max_price - min_price) / bins as f64;
    let mut density = HashMap::new();

    for bar in &series.bars {
        let low_bin = ((bar.low - min_price) / bin_size).floor() as usize;
        let high_bin = ((bar.high - min_price) / bin_size).floor() as usize;

        let low_bin = low_bin.min(bins - 1);
        let high_bin = high_bin.min(bins - 1);

        for bin in low_bin..=high_bin {
            let key = format!(
                "{:.2}-{:.2}",
                min_price + (bin as f64 * bin_size),
                min_price + ((bin + 1) as f64 * bin_size)
            );
            *density.entry(key).or_insert(0) += 1;
        }
    }

    Ok(density)
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_calculate_price_density() {
        let bars = vec![
            Bar {
                symbol: "AAPL".to_string(),
                market: "equities".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 0,
                open: 100.0,
                high: 110.0,
                low: 100.0,
                close: 105.0,
                volume: 1000.0,
            },
            Bar {
                symbol: "AAPL".to_string(),
                market: "equities".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 1,
                open: 105.0,
                high: 120.0,
                low: 105.0,
                close: 115.0,
                volume: 1000.0,
            },
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let density = calculate_price_density(&series, 2).unwrap();
        assert_eq!(density.len(), 2);
    }
}
