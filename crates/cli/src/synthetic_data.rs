//! Provides tools for generating synthetic market data for testing and simulations.
//!
//! This module uses [Geometric Brownian Motion (GBM)](https://en.wikipedia.org/wiki/Geometric_Brownian_motion)
//! to simulate realistic price trajectories. It synthesizes full Open, High, Low, Close (OHLC)
//! bars with randomized volume to allow robust strategy testing without relying solely on historical datasets.
//!
//! # Examples
//!
//! ```
//! use thales_cli::synthetic_data::{generate_synthetic_data, SyntheticDataConfig};
//!
//! let config = SyntheticDataConfig {
//!     symbol: "MOCK".to_string(),
//!     initial_price: 150.0,
//!     drift: 0.0002,      // Slight upward bias
//!     volatility: 0.015,  // Daily volatility
//!     num_bars: 100,      // Generate 100 bars
//!     timeframe: "1d".to_string(),
//!     start_time_ms: 1600000000000,
//! };
//!
//! let series = generate_synthetic_data(config).unwrap();
//! assert_eq!(series.bars.len(), 100);
//! println!("Generated synthetic data for {}", series.bars[0].symbol);
//! ```

use anyhow::Result;
use contracts::{Bar, BarSeries};
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};

/// Configuration for synthetic data generation via Geometric Brownian Motion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticDataConfig {
    /// The ticker symbol for the generated series (e.g., "SYNTH").
    pub symbol: String,
    /// The starting price for the first bar.
    pub initial_price: f64,
    /// The expected return (mu) per time step. A positive value creates an uptrend.
    pub drift: f64,
    /// The standard deviation (sigma) of returns per time step.
    pub volatility: f64,
    /// The total number of OHLC bars to generate.
    pub num_bars: usize,
    /// The string representation of the timeframe (e.g., "1m", "1h", "1d").
    pub timeframe: String,
    /// The UNIX timestamp (ms) for the first bar.
    pub start_time_ms: i64,
}

/// Generates a [`BarSeries`] of synthetic market data using Geometric Brownian Motion.
///
/// # Process
///
/// 1. Iterates `num_bars` times.
/// 2. Calculates the next `Close` price using the GBM formula:
///    $S_{t+1} = S_t \exp\left( (\mu - \frac{\sigma^2}{2})\Delta t + \sigma \sqrt{\Delta t} Z \right)$
///    where $Z$ is a random draw from a standard normal distribution.
/// 3. Approximates `High` and `Low` prices by adding intra-bar noise scaled by the specified `volatility`.
/// 4. Generates a randomized `Volume` centered around 1000.
///
/// # Arguments
///
/// * `config` - The specifications for the generated series.
///
/// # Errors
///
/// Returns an error if `num_bars` is `0`, or if there's an issue initializing the random number generator.
pub fn generate_synthetic_data(config: SyntheticDataConfig) -> Result<BarSeries> {
    if config.num_bars == 0 {
        return Err(anyhow::anyhow!("num_bars must be greater than 0"));
    }

    let mut bars = Vec::with_capacity(config.num_bars);
    let mut rng = rand::thread_rng();
    let normal_dist = Normal::new(0.0, 1.0)?;

    let mut current_price = config.initial_price;
    let mut current_time = config.start_time_ms;

    // Parse timeframe to ms, roughly.
    let time_step_ms: i64 = match config.timeframe.as_str() {
        "1m" => 60_000,
        "5m" => 300_000,
        "15m" => 900_000,
        "1h" => 3_600_000,
        "4h" => 14_400_000,
        "1d" => 86_400_000,
        _ => 86_400_000, // default 1d
    };

    let dt = 1.0; // Assume time step of 1 for the GBM formula per bar.

    for _ in 0..config.num_bars {
        let z = normal_dist.sample(&mut rng);
        // Geometric Brownian Motion step
        // dS = S * (mu * dt + sigma * dW)
        // S_new = S_old * exp((mu - 0.5 * sigma^2) * dt + sigma * sqrt(dt) * Z)
        let drift_term = (config.drift - 0.5 * config.volatility.powi(2)) * dt;
        let vol_term = config.volatility * dt.sqrt() * z;
        let next_price = current_price * (drift_term + vol_term).exp();

        // Create OHLC from the step
        // We simulate intraday volatility for high/low
        let open = current_price;
        let close = next_price;

        let intra_z1 = normal_dist.sample(&mut rng).abs();
        let intra_z2 = normal_dist.sample(&mut rng).abs();

        // High is max(open, close) + random noise scaled by volatility
        let high = open.max(close) * (1.0 + config.volatility * 0.5 * intra_z1);
        // Low is min(open, close) - random noise scaled by volatility
        let low = open.min(close) * (1.0 - config.volatility * 0.5 * intra_z2);

        // Volume is somewhat randomized
        let volume_z = normal_dist.sample(&mut rng).abs();
        let volume = 1000.0 * (1.0 + volume_z);

        bars.push(Bar {
            symbol: config.symbol.clone(),
            market: "synthetic".to_string(),
            timeframe: config.timeframe.clone(),
            timestamp_unix_ms: current_time,
            open,
            high,
            low,
            close,
            volume,
        });

        current_price = close;
        current_time += time_step_ms;
    }

    Ok(BarSeries {
        schema_version: "v0".to_string(),
        bars,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_synthetic_data() {
        let config = SyntheticDataConfig {
            symbol: "TEST".to_string(),
            initial_price: 100.0,
            drift: 0.0001,
            volatility: 0.01,
            num_bars: 10,
            timeframe: "1d".to_string(),
            start_time_ms: 100000,
        };

        let result = generate_synthetic_data(config);
        assert!(result.is_ok());
        let series = result.unwrap();
        assert_eq!(series.bars.len(), 10);
        assert_eq!(series.bars[0].open, 100.0);
    }
}
