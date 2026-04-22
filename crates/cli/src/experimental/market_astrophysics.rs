#![cfg(feature = "nova")]

//! Market Astrophysics Module
//!
//! This module analyzes market data through the lens of astrophysics,
//! calculating velocity, acceleration, and gravitational force based on
//! price movements and volume mass. It identifies celestial states like
//! "Supernova" or "Black Hole" to indicate explosive or implosive market trends.
//!
//! # Examples
//! ```rust
//! #[cfg(feature = "nova")]
//! # {
//! use thales_cli::experimental::market_astrophysics::analyze_astrophysics;
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
//! if let Some(report) = analyze_astrophysics(&series) {
//!     println!("Celestial State: {}", report.celestial_state);
//! }
//! # }
//! ```

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketAstrophysics {
    pub price_velocity: f64,
    pub price_acceleration: f64,
    pub mass: f64,
    pub gravitational_force: f64,
    pub celestial_state: String,
}

pub fn analyze_astrophysics(series: &BarSeries) -> Option<MarketAstrophysics> {
    if series.bars.len() < 2 {
        return None;
    }

    let mut total_mass = 0.0;
    let mut prev_price = series.bars[0].close;
    let mut velocities = Vec::new();

    for bar in series.bars.iter().skip(1) {
        total_mass += bar.volume;
        velocities.push(bar.close - prev_price);
        prev_price = bar.close;
    }

    let price_velocity = if !velocities.is_empty() {
        velocities.iter().sum::<f64>() / velocities.len() as f64
    } else {
        0.0
    };

    let mut accelerations = Vec::new();
    let mut prev_vel = velocities[0];
    for vel in velocities.iter().skip(1) {
        accelerations.push(vel - prev_vel);
        prev_vel = *vel;
    }

    let price_acceleration = if !accelerations.is_empty() {
        accelerations.iter().sum::<f64>() / accelerations.len() as f64
    } else {
        0.0
    };

    // Simplistic gravity based on mass and recent acceleration.
    let gravitational_force = total_mass * price_acceleration.abs();

    let celestial_state = if price_velocity > 0.0 && price_acceleration > 0.0 {
        "Supernova"
    } else if price_velocity < 0.0 && price_acceleration < 0.0 {
        "Black Hole"
    } else {
        "Stable Orbit"
    }
    .to_string();

    Some(MarketAstrophysics {
        price_velocity,
        price_acceleration,
        mass: total_mass,
        gravitational_force,
        celestial_state,
    })
}

pub fn print_ascii_astrophysics(report: &MarketAstrophysics) {
    println!("=== Market Astrophysics Report ===");
    println!("Velocity: {}", report.price_velocity);
    println!("Acceleration: {}", report.price_acceleration);
    println!("Mass (Volume): {}", report.mass);
    println!("Gravitational Force: {}", report.gravitational_force);
    println!("Celestial State: {}", report.celestial_state);
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
    fn test_market_astrophysics_supernova() {
        let mut bars = Vec::new();
        // Base state
        for _ in 0..10 {
            bars.push(create_bar(100.0, 105.0, 100.0));
        }
        // Acceleration phase
        for i in 0..5 {
            bars.push(create_bar(
                105.0 + (i as f64 * 10.0),
                115.0 + (i as f64 * 20.0),
                500.0 + (i as f64 * 100.0),
            ));
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let optics = analyze_astrophysics(&series).expect("Should return astrophysics");

        assert!(optics.price_velocity > 0.0);
        assert!(optics.price_acceleration > 0.0);
        assert_eq!(optics.celestial_state, "Supernova");
    }
}
