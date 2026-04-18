#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// 🌟 Nova: Market Optics
/// Analyzes how price "refracts" and "reflects" like light passing through different mediums.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOptics {
    pub angle_of_incidence: f64,
    pub angle_of_refraction: f64,
    pub refractive_index: f64,
    pub optical_state: String,
}

pub fn analyze_optics(series: &BarSeries) -> Option<MarketOptics> {
    if series.bars.len() < 10 {
        return None;
    }

    let mid = series.bars.len() / 2;
    let first_half = &series.bars[0..mid];
    let second_half = &series.bars[mid..];

    let start_price = first_half[0].close;
    let mid_price = first_half.last()?.close;
    let end_price = second_half.last()?.close;

    let dt1 = first_half.len() as f64;
    let dt2 = second_half.len() as f64;

    let dy1 = mid_price - start_price;
    let dy2 = end_price - mid_price;

    let angle_of_incidence = (dy1 / dt1).atan().to_degrees();
    let angle_of_refraction = (dy2 / dt2).atan().to_degrees();

    let refractive_index = if angle_of_refraction != 0.0 {
        angle_of_incidence / angle_of_refraction
    } else {
        0.0
    };

    let optical_state = if refractive_index < 0.0 {
        "Total Internal Reflection 🪞".to_string()
    } else if refractive_index > 1.0 {
        "Dense Medium (Slowing Down) 🌊".to_string()
    } else if refractive_index > 0.0 && refractive_index < 1.0 {
        "Rare Medium (Speeding Up) 💨".to_string()
    } else {
        "Vacuum (Constant Velocity) 🌌".to_string()
    };

    Some(MarketOptics {
        angle_of_incidence,
        angle_of_refraction,
        refractive_index,
        optical_state,
    })
}

pub fn print_ascii_optics(report: &MarketOptics) {
    println!("🌟 Market Optics Analysis");
    println!("==============================");
    println!("Angle of Incidence:  {:.2}°", report.angle_of_incidence);
    println!("Angle of Refraction: {:.2}°", report.angle_of_refraction);
    println!("Refractive Index:    {:.4}", report.refractive_index);
    println!("Optical State:       {}", report.optical_state);
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
            high: close,
            low: close,
            close,
            volume: 100.0,
        }
    }

    #[test]
    fn test_market_optics_reflection() {
        let mut bars = Vec::new();
        // Goes up, then down (reflection)
        for i in 0..5 {
            bars.push(create_mock_bar(100.0 + (i as f64 * 10.0)));
        }
        for i in 0..5 {
            bars.push(create_mock_bar(140.0 - (i as f64 * 10.0)));
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let optics = analyze_optics(&series).unwrap();
        assert!(optics.refractive_index < 0.0);
        assert_eq!(optics.optical_state, "Total Internal Reflection 🪞");
    }
}
