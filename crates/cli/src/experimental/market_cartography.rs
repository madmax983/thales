#![cfg(feature = "nova")]

//! Market Cartography Module
//!
//! This module maps market data to topographical features. It calculates elevation,
//! slope, and ruggedness to classify the market's current terrain.
//!
//! # Examples
//! ```rust
//! #[cfg(feature = "nova")]
//! # {
//! use thales_cli::experimental::market_cartography::analyze_cartography;
//! use contracts::{BarSeries, Bar};
//!
//! let mut bars = Vec::new();
//! for i in 0..20 {
//!     bars.push(Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: i * 1000, open: 100.0, high: 105.0, low: 95.0, close: 100.0 + i as f64, volume: 1000.0 });
//! }
//! let series = BarSeries { schema_version: "v0".to_string(), bars };
//!
//! if let Some(report) = analyze_cartography(&series) {
//!     println!("Terrain: {}", report.terrain_type);
//! }
//! # }
//! ```

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCartography {
    pub elevation: f64,
    pub slope: f64,
    pub ruggedness: f64,
    pub terrain_type: String,
}

pub fn analyze_cartography(series: &BarSeries) -> Option<MarketCartography> {
    if series.bars.len() < 14 {
        return None;
    }

    let n = series.bars.len();
    let recent = &series.bars[n - 14..n];

    let start_price = recent[0].close;
    let end_price = recent[13].close;

    // Elevation is relative price change
    let elevation = if start_price > 0.0 {
        ((end_price - start_price) / start_price) * 100.0
    } else {
        0.0
    };

    // Slope is average change per bar
    let slope = elevation / 14.0;

    // Ruggedness is related to volatility (high-low spread)
    let mut total_ruggedness = 0.0;
    for bar in recent {
        if bar.low > 0.0 {
            total_ruggedness += (bar.high - bar.low) / bar.low;
        }
    }
    let ruggedness = (total_ruggedness / 14.0) * 100.0;

    let terrain_type = if slope > 0.5 {
        "Mountain Peak 🏔️".to_string()
    } else if slope < -0.5 {
        "Deep Valley 🕳️".to_string()
    } else if ruggedness > 2.0 {
        "Rocky Terrain 🪨".to_string()
    } else {
        "Flat Plains 🌾".to_string()
    };

    Some(MarketCartography {
        elevation,
        slope,
        ruggedness,
        terrain_type,
    })
}

pub fn print_ascii_cartography(report: &MarketCartography) {
    println!("=== Market Cartography ===");
    println!("Terrain Type: {}", report.terrain_type);
    println!("Elevation: {:.2}%", report.elevation);
    println!("Slope: {:.2}%/bar", report.slope);
    println!("Ruggedness (Vol): {:.2}%", report.ruggedness);
    println!("==========================");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_mock_bar(close: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "test".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 0,
            open: close,
            high: close + 0.1,
            low: close - 0.1,
            close,
            volume: 1000.0,
        }
    }

    #[test]
    fn test_market_cartography_plains() {
        let mut bars = Vec::new();
        for _ in 0..14 {
            bars.push(create_mock_bar(100.0));
        }
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let cart = analyze_cartography(&series).unwrap();
        assert_eq!(cart.terrain_type, "Flat Plains 🌾");
    }

    #[test]
    fn test_market_cartography_peak() {
        let mut bars = Vec::new();
        let mut price = 100.0;
        for _ in 0..14 {
            bars.push(create_mock_bar(price));
            price += 1.0;
        }
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let cart = analyze_cartography(&series).unwrap();
        assert_eq!(cart.terrain_type, "Mountain Peak 🏔️");
    }
}
