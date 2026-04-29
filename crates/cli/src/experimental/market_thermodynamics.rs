//! # Market Thermodynamics 🌡️
//!
//! This module applies the laws of thermodynamics to market data, treating price
//! and volume as physical properties of a dynamic system.
//!
//! - **Temperature (Volatility):** Measures the kinetic energy of price movements.
//! - **Entropy (Choppiness):** Measures the disorder or lack of a clear trend.
//! - **Enthalpy (Energy):** Measures the total volume-weighted price movement.
//! - **Free Energy (Momentum):** Measures the energy available to do work (trend).
//!
//! This is an experimental module designed to provide a unique perspective on market
//! conditions, translating financial data into states of matter (Solid, Liquid, Gas, Plasma).

#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Represents the thermodynamic state of a market.
///
/// # Examples
///
/// ```
/// use thales_cli::experimental::market_thermodynamics::ThermodynamicsReport;
///
/// let report = ThermodynamicsReport {
///     temperature: 0.04,
///     entropy: 0.25,
///     enthalpy: 150000.0,
///     free_energy: 112500.0,
///     phase_state: "Plasma (High Volatility, Strong Trend)".to_string(),
/// };
///
/// assert_eq!(report.phase_state, "Plasma (High Volatility, Strong Trend)");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermodynamicsReport {
    pub temperature: f64,
    pub entropy: f64,
    pub enthalpy: f64,
    pub free_energy: f64,
    pub phase_state: String,
}

/// Analyzes a series of market bars to determine their thermodynamic properties.
///
/// This function calculates temperature, entropy, enthalpy, and free energy
/// to classify the market's current "phase state" (e.g., Solid, Liquid, Gas, Plasma).
///
/// # Examples
///
/// ```
/// use contracts::{BarSeries, Bar};
/// use thales_cli::experimental::market_thermodynamics::analyze_thermodynamics;
///
/// let series = BarSeries {
///     schema_version: "v1".to_string(),
///     bars: vec![
///         Bar { symbol: "BTC".to_string(), market: "crypto".to_string(), timeframe: "1d".to_string(), timestamp_unix_ms: 1000, open: 100.0, high: 110.0, low: 90.0, close: 105.0, volume: 1000.0 },
///         Bar { symbol: "BTC".to_string(), market: "crypto".to_string(), timeframe: "1d".to_string(), timestamp_unix_ms: 2000, open: 105.0, high: 115.0, low: 95.0, close: 110.0, volume: 1500.0 },
///     ],
/// };
///
/// let report = analyze_thermodynamics(&series).unwrap();
/// assert!(report.temperature > 0.0);
/// ```
pub fn analyze_thermodynamics(series: &BarSeries) -> Option<ThermodynamicsReport> {
    if series.bars.is_empty() {
        return None;
    }

    let n = series.bars.len() as f64;

    // Temperature: Average True Range relative to close (Volatility)
    let mut temp_sum = 0.0;
    for i in 1..series.bars.len() {
        let bar = &series.bars[i];
        let prev_close = series.bars[i - 1].close;
        let tr = (bar.high - bar.low)
            .max((bar.high - prev_close).abs())
            .max((bar.low - prev_close).abs());
        temp_sum += tr / bar.close;
    }
    let temperature = if n > 1.0 { temp_sum / (n - 1.0) } else { 0.0 };

    // Entropy: Choppiness / lack of direction
    let start_close = series.bars.first()?.close;
    let end_close = series.bars.last()?.close;
    let net_change = (end_close - start_close).abs();

    let mut total_path = 0.0;
    for i in 1..series.bars.len() {
        total_path += (series.bars[i].close - series.bars[i - 1].close).abs();
    }

    // If total path is 0, entropy is 0. If net change is 0 but path > 0, high entropy.
    let entropy = if total_path > 0.0 {
        1.0 - (net_change / total_path)
    } else {
        0.0
    };

    // Enthalpy: Total Volume weighted by Price (Total Energy)
    let mut enthalpy = 0.0;
    for bar in &series.bars {
        enthalpy += bar.volume * bar.close;
    }

    // Gibbs Free Energy: Enthalpy - Temperature * Entropy
    // Here we adapt it: momentum energy available to do work.
    let free_energy = enthalpy * (1.0 - entropy);

    let phase_state = if temperature > 0.05 && entropy < 0.3 {
        "Plasma (High Volatility, Strong Trend)".to_string()
    } else if entropy > 0.7 {
        "Gas (High Entropy, Choppy)".to_string()
    } else if temperature < 0.02 {
        "Solid (Low Volatility, Frozen)".to_string()
    } else {
        "Liquid (Fluid Market)".to_string()
    };

    Some(ThermodynamicsReport {
        temperature,
        entropy,
        enthalpy,
        free_energy,
        phase_state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_thermodynamics_empty() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![],
        };
        assert!(analyze_thermodynamics(&series).is_none());
    }

    #[test]
    fn test_thermodynamics_calculation() {
        let mut bars = Vec::new();
        for i in 0..10 {
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "crypto".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 1000 + i * 1000,
                open: 100.0 + (i as f64),
                high: 105.0 + (i as f64),
                low: 95.0 + (i as f64),
                close: 102.0 + (i as f64),
                volume: 1000.0,
            });
        }
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let report = analyze_thermodynamics(&series).unwrap();
        assert!(report.temperature > 0.0);
        assert!(report.enthalpy > 0.0);
    }
}
