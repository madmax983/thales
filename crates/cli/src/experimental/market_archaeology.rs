#![cfg(feature = "nova")]

//! Market Archaeology Module
//!
//! This module digs into historical market data to unearth "ancient artifacts"
//! (long-forgotten support and resistance levels). It discovers price levels
//! that have been tested multiple times in the past.
//!
//! # Examples
//! ```rust
//! #[cfg(feature = "nova")]
//! # {
//! use thales_cli::experimental::market_archaeology::analyze_archaeology;
//! use contracts::{BarSeries, Bar};
//!
//! let series = BarSeries {
//!     schema_version: "v0".to_string(),
//!     bars: vec![
//!         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 0, open: 100.0, high: 110.0, low: 90.0, close: 100.0, volume: 1000.0 },
//!         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 1000, open: 105.0, high: 115.0, low: 95.0, close: 100.0, volume: 2000.0 },
//!         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 2000, open: 105.0, high: 115.0, low: 95.0, close: 100.0, volume: 2000.0 },
//!     ],
//! };
//!
//! if let Some(report) = analyze_archaeology(&series) {
//!     println!("Artifacts Found: {}", report.artifacts_found);
//! }
//! # }
//! ```

use contracts::BarSeries;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub price_level: f64,
    pub hits: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketArchaeologyReport {
    pub artifacts_found: usize,
    pub artifacts: Vec<Artifact>,
}

pub fn analyze_archaeology(series: &BarSeries) -> Option<MarketArchaeologyReport> {
    if series.bars.is_empty() {
        return None;
    }

    let mut price_counts: HashMap<i64, usize> = HashMap::new();

    for bar in &series.bars {
        let rounded_close = bar.close.round() as i64;
        *price_counts.entry(rounded_close).or_insert(0) += 1;
    }

    let mut artifacts: Vec<Artifact> = price_counts
        .into_iter()
        .filter(|&(_, hits)| hits > 1) // Only keep levels tested more than once
        .map(|(price, hits)| Artifact {
            price_level: price as f64,
            hits,
        })
        .collect();

    artifacts.sort_by(|a, b| b.hits.cmp(&a.hits)); // Sort by most hits

    Some(MarketArchaeologyReport {
        artifacts_found: artifacts.len(),
        artifacts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_archaeology_dig() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![
                Bar {
                    symbol: "TEST".into(),
                    market: "test".into(),
                    timeframe: "1d".into(),
                    timestamp_unix_ms: 0,
                    open: 100.0,
                    high: 110.0,
                    low: 90.0,
                    close: 105.0,
                    volume: 1000.0,
                },
                Bar {
                    symbol: "TEST".into(),
                    market: "test".into(),
                    timeframe: "1d".into(),
                    timestamp_unix_ms: 1000,
                    open: 105.0,
                    high: 115.0,
                    low: 95.0,
                    close: 105.0,
                    volume: 2000.0,
                },
                Bar {
                    symbol: "TEST".into(),
                    market: "test".into(),
                    timeframe: "1d".into(),
                    timestamp_unix_ms: 2000,
                    open: 105.0,
                    high: 115.0,
                    low: 95.0,
                    close: 110.0,
                    volume: 2000.0,
                },
            ],
        };

        let report = analyze_archaeology(&series).unwrap();
        assert_eq!(report.artifacts_found, 1);
        assert_eq!(report.artifacts[0].price_level, 105.0);
        assert_eq!(report.artifacts[0].hits, 2);
    }
}
