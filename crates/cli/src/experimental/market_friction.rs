#![cfg(feature = "nova")]

//! Market Friction Module
//!
//! This module measures the resistance to price movement by comparing
//! volume to price range. High friction indicates a lot of effort (volume)
//! for little result (price movement), which can signal accumulation or distribution.
//!
//! # Examples
//! ```rust
//! #[cfg(feature = "nova")]
//! # {
//! use thales_cli::experimental::market_friction::analyze_friction;
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
//! if let Some(report) = analyze_friction(&series) {
//!     println!("Current Friction: {}", report.current_friction);
//! }
//! # }
//! ```


use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// 🌟 Nova: Market Friction
/// Measures the resistance to price movement by comparing volume to price range.
/// High friction = lots of effort (volume) for little result (price movement).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketFriction {
    pub average_friction: f64,
    pub current_friction: f64,
    pub friction_trend: String,
    pub regime: String,
}

pub fn analyze_friction(series: &BarSeries) -> Option<MarketFriction> {
    if series.bars.is_empty() {
        return None;
    }

    let mut frictions = Vec::new();

    for bar in &series.bars {
        let price_range = bar.high - bar.low;
        let friction = if price_range > 0.0 {
            bar.volume / price_range
        } else {
            bar.volume // Infinite friction if no movement
        };
        frictions.push(friction);
    }

    if frictions.is_empty() {
        return None;
    }

    let average_friction: f64 = frictions.iter().sum::<f64>() / frictions.len() as f64;
    let current_friction = *frictions.last().unwrap();

    let friction_trend = if current_friction > average_friction {
        "Increasing".to_string()
    } else {
        "Decreasing".to_string()
    };

    let regime = if current_friction > average_friction * 1.5 {
        "High Friction (Accumulation/Distribution)".to_string()
    } else if current_friction < average_friction * 0.5 {
        "Low Friction (Free Fall / Melt Up)".to_string()
    } else {
        "Normal Friction".to_string()
    };

    Some(MarketFriction {
        average_friction,
        current_friction,
        friction_trend,
        regime,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_market_friction_basic() {
        let bars = vec![
            Bar {
                symbol: "TEST".into(),
                market: "test".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 0,
                open: 100.0,
                high: 110.0,
                low: 90.0,
                close: 105.0,
                volume: 1000.0, // range = 20, friction = 50
            },
            Bar {
                symbol: "TEST".into(),
                market: "test".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 1,
                open: 105.0,
                high: 106.0,
                low: 104.0,
                close: 105.0,
                volume: 2000.0, // range = 2, friction = 1000
            },
        ];
        let series = BarSeries {
            schema_version: "v0".into(),
            bars,
        };

        let report = analyze_friction(&series).expect("Failed to calculate friction");
        assert_eq!(report.current_friction, 1000.0);
        assert_eq!(report.average_friction, 525.0);
        assert_eq!(report.friction_trend, "Increasing");
        assert_eq!(report.regime, "High Friction (Accumulation/Distribution)");
    }
}
