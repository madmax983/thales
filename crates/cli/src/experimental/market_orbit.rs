#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// 🌟 Nova: Market Orbit
/// Translates market conditions into orbital mechanics.
///
/// - Sun (Center of Mass): Simple Moving Average (50 periods).
/// - Planet (Current Price): The last close price.
/// - Orbital Distance: Distance from the Sun.
/// - Orbital Velocity: Rate of change of the distance.
/// - Trajectory: Stable Orbit, Escape Trajectory, Orbital Decay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOrbit {
    pub orbital_distance: f64,
    pub orbital_velocity: f64,
    pub eccentricity: f64,
    pub trajectory: String,
}

pub fn analyze_orbit(series: &BarSeries) -> Option<MarketOrbit> {
    if series.bars.len() < 51 {
        return None;
    }

    let n = series.bars.len();

    // Calculate Sun (Center of Mass) = 50 SMA for current bar
    let mut current_sum = 0.0;
    for bar in &series.bars[n - 50..n] {
        current_sum += bar.close;
    }
    let current_sun_price = current_sum / 50.0;

    // Calculate Sun for previous bar
    let mut prev_sum = 0.0;
    for bar in &series.bars[n - 51..n - 1] {
        prev_sum += bar.close;
    }
    let prev_sun_price = prev_sum / 50.0;

    let planet_price = series.bars[n - 1].close;
    let prev_planet_price = series.bars[n - 2].close;

    // Orbital Distance (percentage from Sun)
    let orbital_distance = if current_sun_price > 0.0 {
        ((planet_price - current_sun_price) / current_sun_price) * 100.0
    } else {
        0.0
    };

    // Orbital Velocity (percentage change in distance)
    let prev_orbital_distance = if prev_sun_price > 0.0 {
        ((prev_planet_price - prev_sun_price) / prev_sun_price) * 100.0
    } else {
        0.0
    };

    let orbital_velocity = orbital_distance - prev_orbital_distance;

    // Eccentricity (Volatility / Distance)
    let mut total_volatility = 0.0;
    for bar in &series.bars[n - 14..n] {
        // 14 period volatility
        if bar.low > 0.0 {
            total_volatility += (bar.high - bar.low) / bar.low;
        }
    }
    let eccentricity = total_volatility / 14.0 * 100.0; // Scaled

    // Trajectory
    let trajectory = if orbital_velocity > 2.0 && orbital_distance > 5.0 {
        "Escape Trajectory 🚀".to_string()
    } else if orbital_velocity < -2.0 && orbital_distance < -5.0 {
        "Gravitational Collapse \u{1f573}\u{fe0f}".to_string() // Hex hole emoji
    } else if orbital_velocity.abs() < 1.0 && orbital_distance.abs() < 5.0 {
        "Stable Circular Orbit \u{1f30d}".to_string() // Earth globe emoji
    } else if eccentricity > 5.0 {
        "Highly Eccentric Orbit \u{2604}\u{fe0f}".to_string() // Comet emoji
    } else if orbital_distance < 0.0 && orbital_velocity > 0.0 {
        "Periapsis Approach \u{2600}\u{fe0f}".to_string() // Sun emoji
    } else if orbital_distance > 0.0 && orbital_velocity < 0.0 {
        "Apoapsis Return \u{1f311}".to_string() // New moon emoji
    } else {
        "Elliptical Orbit \u{1fa90}".to_string() // Ringed planet emoji
    };

    Some(MarketOrbit {
        orbital_distance,
        orbital_velocity,
        eccentricity,
        trajectory,
    })
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
            high: close + 1.0,
            low: close - 1.0,
            close,
            volume: 100.0,
        }
    }

    #[test]
    fn test_market_orbit() {
        let mut bars = Vec::new();
        // 50 bars of 100.0
        for _ in 0..50 {
            bars.push(create_mock_bar(100.0));
        }
        // Huge spike
        bars.push(create_mock_bar(110.0));

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let orbit = analyze_orbit(&series).unwrap();
        assert!(orbit.orbital_distance > 5.0); // (110 - ~100) / 100 * 100 = ~10.0
        assert!(orbit.orbital_velocity > 2.0);
        assert_eq!(orbit.trajectory, "Escape Trajectory 🚀");
    }
}
