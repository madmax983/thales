#![cfg(feature = "nova")]

//! Market Energy Physics
//!
//! This module attempts to apply classical physics concepts to market movements.
//! It conceptualizes price changes relative to a moving average as "Potential Energy" (height)
//! and the velocity of price changes combined with volume (mass) as "Kinetic Energy".
//!
//! By framing market movements in these terms, we can look for exhaustion points (where
//! potential energy is high but kinetic energy drops) or explosive moves (where kinetic
//! energy rapidly increases).

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Configuration for Market Energy Analysis.
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::market_energy::EnergyConfig;
///
/// let config = EnergyConfig {
///     window: 14,
/// };
///
/// assert_eq!(config.window, 14);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyConfig {
    /// The number of periods to calculate the moving average for potential energy.
    pub window: usize,
}

/// A report detailing the current market "energy" state.
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::market_energy::EnergyReport;
///
/// let report = EnergyReport {
///     potential_energy: 150.5,
///     kinetic_energy: 5000.0,
///     total_energy: 5150.5,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyReport {
    /// Energy derived from the price's distance from the moving average.
    pub potential_energy: f64,
    /// Energy derived from price velocity and volume (mass).
    pub kinetic_energy: f64,
    /// The sum of potential and kinetic energy.
    pub total_energy: f64,
}

/// Analyzes a series of market bars to calculate their current potential, kinetic, and total energy.
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::market_energy::{analyze_energy, EnergyConfig};
/// use contracts::{BarSeries, Bar};
///
/// let bars = vec![
///     Bar {
///         symbol: "BTC".to_string(),
///         market: "crypto".to_string(),
///         timeframe: "1d".to_string(),
///         timestamp_unix_ms: 0,
///         open: 100.0, high: 100.0, low: 100.0, close: 100.0, volume: 100.0,
///     },
///     Bar {
///         symbol: "BTC".to_string(),
///         market: "crypto".to_string(),
///         timeframe: "1d".to_string(),
///         timestamp_unix_ms: 1,
///         open: 105.0, high: 105.0, low: 105.0, close: 105.0, volume: 100.0,
///     },
///     Bar {
///         symbol: "BTC".to_string(),
///         market: "crypto".to_string(),
///         timeframe: "1d".to_string(),
///         timestamp_unix_ms: 2,
///         open: 115.0, high: 115.0, low: 115.0, close: 115.0, volume: 100.0,
///     },
/// ];
///
/// let series = BarSeries { schema_version: "v0".to_string(), bars };
/// let config = EnergyConfig { window: 3 };
///
/// let report = analyze_energy(&series, config).unwrap();
/// assert!(report.total_energy > 0.0);
/// ```
pub fn analyze_energy(series: &BarSeries, config: EnergyConfig) -> anyhow::Result<EnergyReport> {
    if series.bars.len() < config.window || config.window < 2 {
        return Ok(EnergyReport {
            potential_energy: 0.0,
            kinetic_energy: 0.0,
            total_energy: 0.0,
        });
    }

    let n = series.bars.len();

    // Potential Energy ~ Height (Price relative to moving average)
    // Kinetic Energy ~ Velocity squared (Price change squared)

    let mut sum = 0.0;
    for i in (n - config.window)..n {
        sum += series.bars[i].close;
    }
    let sma = sum / config.window as f64;

    let current_price = series.bars[n - 1].close;
    let pe = (current_price - sma).abs() * 9.81; // 9.81 is gravity constant for fun

    let p1 = series.bars[n - 1].close;
    let p0 = series.bars[n - 2].close;
    let v = p1 - p0;
    let mass = series.bars[n - 1].volume; // Volume as mass

    let ke = 0.5 * mass * v * v;

    Ok(EnergyReport {
        potential_energy: pe,
        kinetic_energy: ke,
        total_energy: pe + ke,
    })
}

/// Prints a simple ASCII representation of the calculated market energy.
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::market_energy::{print_ascii_energy, EnergyReport};
///
/// let report = EnergyReport {
///     potential_energy: 10.5,
///     kinetic_energy: 200.0,
///     total_energy: 210.5,
/// };
///
/// print_ascii_energy(&report);
/// ```
pub fn print_ascii_energy(report: &EnergyReport) {
    println!("=== Market Energy Physics ===");
    println!("Potential Energy: {:.2}", report.potential_energy);
    println!("Kinetic Energy:   {:.2}", report.kinetic_energy);
    println!("Total Energy:     {:.2}", report.total_energy);
    println!("=============================");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_mock_bar(close: f64, volume: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "test".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 0,
            open: close,
            high: close,
            low: close,
            close,
            volume,
        }
    }

    #[test]
    fn test_analyze_energy() {
        let bars = vec![
            create_mock_bar(100.0, 100.0),
            create_mock_bar(105.0, 100.0),
            create_mock_bar(115.0, 100.0),
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = EnergyConfig { window: 3 };
        let report = analyze_energy(&series, config).unwrap();

        // SMA = (100 + 105 + 115) / 3 = 106.66
        // PE = |115 - 106.66| * 9.81 = 81.75
        // KE = 0.5 * 100 * (115 - 105)^2 = 0.5 * 100 * 100 = 5000

        assert!(report.potential_energy > 0.0);
        assert!(report.kinetic_energy > 0.0);
    }
}
