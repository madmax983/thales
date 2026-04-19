#![cfg(feature = "nova")]

//! 🌟 Nova: Market Alchemy Module
//!
//! This module interprets market data through the lens of ancient alchemy.
//! It attempts to "transmute" raw market data into high-confidence signals
//! by balancing the Four Elements:
//!
//! - **Earth (Support/Resistance):** The solidity of the price floor.
//! - **Water (Liquidity/Volume):** The flow and depth of the market.
//! - **Air (Volatility):** The rapid, expansive movements in price.
//! - **Fire (Momentum):** The directional energy and heat of the trend.
//!
//! When the elements are in perfect alignment, a "Philosopher's Stone" signal is generated!

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlchemicalState {
    pub earth_stability: f64,
    pub water_flow: f64,
    pub air_volatility: f64,
    pub fire_momentum: f64,
    pub transmutation_ready: bool, // True if a Golden Signal is found
    pub dominant_element: String,
}

/// Calculates the alchemical state of a given series of bars.
pub fn calculate_alchemy(series: &BarSeries) -> Option<AlchemicalState> {
    if series.bars.len() < 14 {
        return None;
    }

    let len = series.bars.len();
    let recent_bars = &series.bars[len - 14..];

    // Earth: How close are we to the local minimum (Support)?
    let mut lowest_low = f64::MAX;
    for bar in recent_bars {
        if bar.low < lowest_low {
            lowest_low = bar.low;
        }
    }
    let current_close = recent_bars.last().unwrap().close;
    // Lower value means closer to support (more Earth stability)
    let earth_stability = 100.0 / (1.0 + ((current_close - lowest_low) / lowest_low).abs());

    // Water: Average volume
    let mut total_volume = 0.0;
    for bar in recent_bars {
        total_volume += bar.volume;
    }
    let water_flow = total_volume / 14.0;

    // Air: Average True Range (roughly) or high-low spread
    let mut total_spread = 0.0;
    for bar in recent_bars {
        total_spread += bar.high - bar.low;
    }
    let air_volatility = total_spread / 14.0;

    // Fire: Momentum (Current Close - Close 14 periods ago)
    let past_close = recent_bars[0].close;
    let fire_momentum = current_close - past_close;

    let mut dominant = "Earth";
    let mut max_score = earth_stability;

    // We normalize them roughly to find the dominant element just for fun
    let water_score = water_flow % 100.0; // Arbitrary normalization for demonstration
    let air_score = air_volatility * 10.0;
    let fire_score = fire_momentum.abs() * 5.0;

    if water_score > max_score {
        dominant = "Water";
        max_score = water_score;
    }
    if air_score > max_score {
        dominant = "Air";
        max_score = air_score;
    }
    if fire_score > max_score {
        dominant = "Fire";
    }

    // Transmutation (The Golden Signal):
    // If momentum is positive (Fire), volatility is contained (Air), we are bouncing off support (Earth), and volume is decent (Water).
    let transmutation_ready = fire_momentum > 0.0 && earth_stability > 80.0 && water_flow > 0.0;

    Some(AlchemicalState {
        earth_stability,
        water_flow,
        air_volatility,
        fire_momentum,
        transmutation_ready,
        dominant_element: dominant.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_mock_bar(low: f64, high: f64, close: f64, volume: f64) -> Bar {
        Bar {
            symbol: "GOLD".to_string(),
            market: "commodities".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 0,
            open: low,
            high,
            low,
            close,
            volume,
        }
    }

    #[test]
    fn test_alchemy_transmutation() {
        let mut bars = Vec::new();
        let mut price = 100.0;
        for i in 0..13 {
            bars.push(create_mock_bar(price - 1.0, price + 1.0, price, 1000.0));
            // Slight dip to create a floor
            if i > 5 {
                price -= 0.5;
            }
        }
        // Final bar pushes up from the floor
        bars.push(create_mock_bar(
            price - 1.0,
            price + 5.0,
            price + 4.0,
            5000.0,
        ));

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let state = calculate_alchemy(&series).unwrap();
        assert!(state.fire_momentum > 0.0);
        assert!(state.transmutation_ready); // We are bouncing off support
    }

    #[test]
    fn test_not_enough_bars() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![create_mock_bar(10.0, 12.0, 11.0, 100.0)],
        };
        assert!(calculate_alchemy(&series).is_none());
    }
}
