//! Black Swan Simulation
//!
//! This module provides tools to simulate "Black Swan" events on historical or synthetic
//! price data. It helps answer the question: "How would my strategy react if the market
//! suddenly collapsed by 30% in a single day, or if volatility spiked 10x?"
//!
//! # Supported Events
//! - **Flash Crash:** A sudden, massive drop in price over a short period.
//! - **Volatility Spike:** A period of extreme price fluctuation (widening high/low spreads).
//! - **Liquidity Freeze:** A period where volume drops to near-zero and prices gap.

use anyhow::Result;
use contracts::BarSeries;
use rand::SeedableRng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};

/// The type of Black Swan event to simulate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BlackSwanEvent {
    /// A sudden drop in price.
    FlashCrash {
        /// Percentage drop (e.g., 0.30 for a 30% drop).
        drop_pct: f64,
        /// Number of bars the crash takes to fully play out.
        duration_bars: usize,
    },
    /// An extreme increase in volatility.
    VolatilitySpike {
        /// Multiplier for the high/low range (e.g., 5.0 for 5x volatility).
        multiplier: f64,
        /// Number of bars the spike lasts.
        duration_bars: usize,
    },
    /// A sudden drop in volume and unpredictable price gapping.
    LiquidityFreeze {
        /// Volume multiplier (e.g., 0.01 for a 99% drop in volume).
        volume_multiplier: f64,
        /// Number of bars the freeze lasts.
        duration_bars: usize,
    },
}

/// Configuration for injecting a Black Swan event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackSwanConfig {
    /// The event to simulate.
    pub event: BlackSwanEvent,
    /// The index of the bar where the event should begin.
    pub start_index: usize,
    /// Optional seed for deterministic gapping during a Liquidity Freeze.
    pub seed: Option<u64>,
}

/// Injects a Black Swan event into a BarSeries.
///
/// Returns a new `BarSeries` with the simulated event applied, leaving the original unchanged.
pub fn inject_black_swan(series: &BarSeries, config: BlackSwanConfig) -> Result<BarSeries> {
    let mut modified_bars = series.bars.clone();
    let n = modified_bars.len();

    if config.start_index >= n {
        return Ok(BarSeries {
            schema_version: series.schema_version.clone(),
            bars: modified_bars,
        });
    }

    let mut rng = if let Some(seed) = config.seed {
        rand::rngs::StdRng::seed_from_u64(seed)
    } else {
        rand::rngs::StdRng::from_entropy()
    };

    match config.event {
        BlackSwanEvent::FlashCrash {
            drop_pct,
            duration_bars,
        } => {
            let end_index = (config.start_index + duration_bars).min(n);
            let drop_per_bar = drop_pct / (duration_bars as f64);
            let mut cumulative_drop = 0.0;

            for bar in modified_bars
                .iter_mut()
                .take(end_index)
                .skip(config.start_index)
            {
                cumulative_drop += drop_per_bar;
                let multiplier = 1.0 - cumulative_drop;
                bar.open *= multiplier;
                bar.high *= multiplier;
                bar.low *= multiplier;
                bar.close *= multiplier;
            }

            // Maintain the crash level for the rest of the series
            if end_index < n {
                let final_multiplier = 1.0 - drop_pct;
                for bar in modified_bars.iter_mut().take(n).skip(end_index) {
                    bar.open *= final_multiplier;
                    bar.high *= final_multiplier;
                    bar.low *= final_multiplier;
                    bar.close *= final_multiplier;
                }
            }
        }
        BlackSwanEvent::VolatilitySpike {
            multiplier,
            duration_bars,
        } => {
            let end_index = (config.start_index + duration_bars).min(n);
            for bar in modified_bars
                .iter_mut()
                .take(end_index)
                .skip(config.start_index)
            {
                let mid = (bar.high + bar.low) / 2.0;
                let half_range = (bar.high - bar.low) / 2.0;

                let new_half_range = half_range * multiplier;
                bar.high = mid + new_half_range;
                bar.low = (mid - new_half_range).max(0.0);
            }
        }
        BlackSwanEvent::LiquidityFreeze {
            volume_multiplier,
            duration_bars,
        } => {
            let end_index = (config.start_index + duration_bars).min(n);
            let normal_dist = Normal::new(0.0, 0.05).unwrap();

            for bar in modified_bars
                .iter_mut()
                .take(end_index)
                .skip(config.start_index)
            {
                bar.volume *= volume_multiplier;

                // Add random gaps up to 5% standard deviation during freeze
                let gap_pct = normal_dist.sample(&mut rng);
                let gap_multiplier = 1.0 + gap_pct;

                bar.open *= gap_multiplier;
                bar.high *= gap_multiplier;
                bar.low *= gap_multiplier;
                bar.close *= gap_multiplier;
            }
        }
    }

    Ok(BarSeries {
        schema_version: series.schema_version.clone(),
        bars: modified_bars,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_dummy_series() -> BarSeries {
        let mut bars = Vec::new();
        for i in 0..100 {
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "crypto".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: i as i64 * 86400000,
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 100.0,
                volume: 1000.0,
            });
        }
        BarSeries {
            schema_version: "v0".to_string(),
            bars,
        }
    }

    #[test]
    fn test_flash_crash() {
        let series = create_dummy_series();
        let config = BlackSwanConfig {
            event: BlackSwanEvent::FlashCrash {
                drop_pct: 0.50,
                duration_bars: 5,
            },
            start_index: 50,
            seed: None,
        };

        let modified = inject_black_swan(&series, config).unwrap();

        // Before crash
        assert_eq!(modified.bars[49].close, 100.0);

        // After crash completes (should be 50% of 100.0)
        assert_eq!(modified.bars[54].close, 50.0);

        // Ensure price stays down
        assert_eq!(modified.bars[60].close, 50.0);
    }

    #[test]
    fn test_volatility_spike() {
        let series = create_dummy_series();
        let config = BlackSwanConfig {
            event: BlackSwanEvent::VolatilitySpike {
                multiplier: 3.0,
                duration_bars: 10,
            },
            start_index: 20,
            seed: None,
        };

        let modified = inject_black_swan(&series, config).unwrap();

        // Normal volatility range is 10 (105 - 95)
        let normal_range = modified.bars[19].high - modified.bars[19].low;
        assert_eq!(normal_range, 10.0);

        // Spiked volatility range should be 30 (3.0 * 10)
        let spiked_range = modified.bars[25].high - modified.bars[25].low;
        assert_eq!(spiked_range, 30.0);

        // Return to normal
        let post_range = modified.bars[35].high - modified.bars[35].low;
        assert_eq!(post_range, 10.0);
    }

    #[test]
    fn test_liquidity_freeze() {
        let series = create_dummy_series();
        let config = BlackSwanConfig {
            event: BlackSwanEvent::LiquidityFreeze {
                volume_multiplier: 0.05,
                duration_bars: 5,
            },
            start_index: 80,
            seed: Some(42),
        };

        let modified = inject_black_swan(&series, config).unwrap();

        // Normal volume
        assert_eq!(modified.bars[79].volume, 1000.0);

        // Frozen volume
        assert_eq!(modified.bars[82].volume, 50.0);

        // Return to normal volume
        assert_eq!(modified.bars[86].volume, 1000.0);
    }
}
