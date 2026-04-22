#![cfg(feature = "nova")]

//! Market Optics Module
//!
//! This module applies optical principles like refraction and dispersion
//! to market trends. By splitting a time series into two mediums, it
//! analyzes how a trend "bends" or disperses based on changes in volume density.
//!
//! # Examples
//! ```rust
//! #[cfg(feature = "nova")]
//! # {
//! use thales_cli::experimental::market_optics::analyze_optics;
//! use contracts::{BarSeries, Bar};
//!
//! let mut bars = Vec::new();
//! for i in 0..10 {
//!     bars.push(Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: i * 1000, open: 100.0, high: 105.0, low: 95.0, close: 100.0, volume: 1000.0 });
//! }
//! let series = BarSeries { schema_version: "v0".to_string(), bars };
//!
//! if let Some(report) = analyze_optics(&series) {
//!     println!("Optical State: {}", report.optical_state);
//! }
//! # }
//! ```

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOptics {
    pub refractive_index: f64,
    pub incident_trend_angle: f64,
    pub refracted_trend_angle: f64,
    pub dispersion_factor: f64,
    pub optical_state: String,
}

pub fn analyze_optics(series: &BarSeries) -> Option<MarketOptics> {
    if series.bars.len() < 10 {
        return None;
    }

    let mid = series.bars.len() / 2;
    let medium_1 = &series.bars[..mid];
    let medium_2 = &series.bars[mid..];

    let vol_1: f64 = medium_1.iter().map(|b| b.volume).sum();
    let vol_2: f64 = medium_2.iter().map(|b| b.volume).sum();

    let refractive_index = if vol_1 > 0.0 { vol_2 / vol_1 } else { 1.0 };

    let price_change_1 = medium_1.last()?.close - medium_1.first()?.open;
    let incident_trend_angle = price_change_1.atan();

    let price_change_2 = medium_2.last()?.close - medium_2.first()?.open;
    let refracted_trend_angle = price_change_2.atan();

    let dispersion_factor = (incident_trend_angle - refracted_trend_angle).abs() * refractive_index;

    let optical_state =
        if refractive_index > 1.5 && refracted_trend_angle.abs() < incident_trend_angle.abs() {
            "Total Internal Reflection"
        } else if dispersion_factor > 0.5 {
            "High Dispersion"
        } else {
            "Clear Propagation"
        }
        .to_string();

    Some(MarketOptics {
        refractive_index,
        incident_trend_angle,
        refracted_trend_angle,
        dispersion_factor,
        optical_state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_bar(open: f64, close: f64, volume: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "test".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 0,
            open,
            high: close.max(open),
            low: close.min(open),
            close,
            volume,
        }
    }

    #[test]
    fn test_market_optics() {
        let mut bars = Vec::new();
        for i in 0..5 {
            bars.push(create_bar(100.0 + i as f64, 101.0 + i as f64, 1000.0));
        }
        for i in 0..5 {
            bars.push(create_bar(
                105.0 + (i as f64 * 0.1),
                105.1 + (i as f64 * 0.1),
                5000.0,
            ));
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let optics = analyze_optics(&series).expect("Should return optics");

        assert!(optics.refractive_index > 1.0);
        assert_eq!(optics.optical_state, "Total Internal Reflection");
    }
}
