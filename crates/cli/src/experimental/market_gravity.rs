#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// 🌟 Nova: Market Gravity
/// Calculates the "gravity" of different price levels by tracking how much volume was traded at each price.
/// High gravity = strong support/resistance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketGravity {
    pub levels: Vec<GravityLevel>,
    pub center_of_mass: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GravityLevel {
    pub price: f64,
    pub mass: f64, // Total volume at this level
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketGravityConfig {
    pub num_bins: usize,
}

pub fn calculate_gravity(series: &BarSeries, config: MarketGravityConfig) -> Option<MarketGravity> {
    if series.bars.is_empty() || config.num_bins == 0 {
        return None;
    }

    let min_price = series
        .bars
        .iter()
        .map(|b| b.low)
        .fold(f64::INFINITY, f64::min);
    let max_price = series
        .bars
        .iter()
        .map(|b| b.high)
        .fold(f64::NEG_INFINITY, f64::max);

    if max_price <= min_price {
        return None;
    }

    let bin_size = (max_price - min_price) / config.num_bins as f64;
    let mut bins = vec![0.0; config.num_bins];
    let mut total_mass = 0.0;
    let mut mass_moment = 0.0;

    for bar in &series.bars {
        let bar_range = bar.high - bar.low;
        if bar_range <= 0.0 {
            continue;
        }

        for (i, bin) in bins.iter_mut().enumerate() {
            let bin_bottom = min_price + (i as f64) * bin_size;
            let bin_top = bin_bottom + bin_size;

            let overlap_bottom = bar.low.max(bin_bottom);
            let overlap_top = bar.high.min(bin_top);

            if overlap_top > overlap_bottom {
                let overlap_ratio = (overlap_top - overlap_bottom) / bar_range;
                let volume_in_bin = bar.volume * overlap_ratio;
                *bin += volume_in_bin;

                let bin_center = (bin_bottom + bin_top) / 2.0;
                total_mass += volume_in_bin;
                mass_moment += volume_in_bin * bin_center;
            }
        }
    }

    let center_of_mass = if total_mass > 0.0 {
        mass_moment / total_mass
    } else {
        (min_price + max_price) / 2.0
    };

    let mut levels = Vec::new();
    for (i, &mass) in bins.iter().enumerate() {
        levels.push(GravityLevel {
            price: min_price + (i as f64 + 0.5) * bin_size,
            mass,
        });
    }

    Some(MarketGravity {
        levels,
        center_of_mass,
    })
}

pub fn print_ascii_gravity(report: &MarketGravity) {
    println!("🌟 Market Gravity Analysis");
    println!("==============================");
    println!("Center of Mass: {:.4}", report.center_of_mass);
    println!();

    let max_mass = report.levels.iter().map(|l| l.mass).fold(0.0, f64::max);

    for level in report.levels.iter().rev() {
        let bar_len = if max_mass > 0.0 {
            ((level.mass / max_mass) * 40.0) as usize
        } else {
            0
        };
        let bar: String = "█".repeat(bar_len);
        let marker = if (level.price - report.center_of_mass).abs() < (level.price * 0.01) {
            " < COM"
        } else {
            ""
        };
        println!(
            "{:10.2} | {:40} {:.1}{}",
            level.price, bar, level.mass, marker
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_market_gravity_basic() {
        let bars = vec![
            Bar {
                symbol: "TEST".into(),
                market: "test".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 0,
                open: 10.0,
                high: 20.0,
                low: 5.0,
                close: 15.0,
                volume: 100.0,
            },
            Bar {
                symbol: "TEST".into(),
                market: "test".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 1,
                open: 15.0,
                high: 25.0,
                low: 10.0,
                close: 20.0,
                volume: 200.0,
            },
        ];
        let series = BarSeries {
            schema_version: "v0".into(),
            bars,
        };
        let config = MarketGravityConfig { num_bins: 5 };
        let report = calculate_gravity(&series, config).expect("Failed to calculate gravity");
        assert!(report.center_of_mass > 10.0);
        assert_eq!(report.levels.len(), 5);
    }
}
