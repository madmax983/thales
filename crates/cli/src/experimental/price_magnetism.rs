#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceMagnetismReport {
    pub current_price: f64,
    pub net_magnetic_force: f64,
}

pub fn analyze_magnetism(series: &BarSeries) -> Option<PriceMagnetismReport> {
    if series.bars.is_empty() {
        return None;
    }

    let current_price = series.bars.last()?.close;

    // Simplistic gravity/magnetism algorithm to nearby round numbers:
    // E.g. if price is 100.5, the nearest round number might be 100.0
    // We consider round numbers to be multiples of 10.0 for this simple prototype
    let nearest_round = (current_price / 10.0).round() * 10.0;

    // Magnetic force is proportional to distance but pulls towards the round number
    // Positive means upward pull, negative means downward pull
    let net_magnetic_force = nearest_round - current_price;

    Some(PriceMagnetismReport {
        current_price,
        net_magnetic_force,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_analyze_magnetism_calculates_correct_force() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![Bar {
                symbol: "TEST".into(),
                market: "test".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 0,
                open: 10.0,
                high: 20.0,
                low: 5.0,
                close: 100.5,
                volume: 100.0,
            }],
        };

        let report_opt = analyze_magnetism(&series);
        assert!(report_opt.is_some());
        if let Some(report) = report_opt {
            assert_eq!(report.current_price, 100.5);
            assert!(report.net_magnetic_force < 0.0);
        }
    }
}
