//! Market Quantum Mechanics Module
//!
//! Applies quantum mechanical concepts to market data.
//! Treats price as a probability wave that collapses upon observation (the close).
//!
//! Key Concepts:
//! - Wave Function (Ψ): The probability distribution of price.
//! - Uncertainty Principle: Relationship between price precision and momentum precision.
//! - Tunneling: Probability of breaking through strong support/resistance.

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumConfig {
    pub planck_constant_analog: f64,
}

impl Default for QuantumConfig {
    fn default() -> Self {
        Self {
            planck_constant_analog: 0.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumReport {
    pub wave_function_collapse_probability: f64,
    pub price_momentum_uncertainty: f64,
    pub tunneling_probability: f64,
    pub energy_level: usize,
}

pub fn analyze_quantum(series: &BarSeries, config: QuantumConfig) -> Option<QuantumReport> {
    if series.bars.len() < 2 {
        return None;
    }

    let mut total_uncertainty = 0.0;
    let mut tunneling_events = 0;

    // Simple analog: High-Low range is positional uncertainty (Δx)
    // Volume * (Close - Open) is momentum (Δp)
    // We expect Δx * Δp >= h_bar / 2

    for window in series.bars.windows(2) {
        let prev = &window[0];
        let curr = &window[1];

        let delta_x = curr.high - curr.low;
        let momentum = curr.volume * (curr.close - curr.open).abs();

        let uncertainty = delta_x * momentum;
        total_uncertainty += uncertainty;

        // Tunneling: If price gaps through the previous high/low without trading within the range
        if (curr.open > prev.high && curr.low > prev.high)
            || (curr.open < prev.low && curr.high < prev.low)
        {
            tunneling_events += 1;
        }
    }

    let avg_uncertainty = total_uncertainty / (series.bars.len() as f64 - 1.0);
    let tunneling_prob = tunneling_events as f64 / (series.bars.len() as f64 - 1.0);

    // Energy level: quantized volatility (e.g. integer steps of ATR)
    let latest_range = series.bars.last().unwrap().high - series.bars.last().unwrap().low;
    let energy_level = (latest_range / config.planck_constant_analog).floor() as usize;

    Some(QuantumReport {
        wave_function_collapse_probability: 1.0, // Classical observation always collapses
        price_momentum_uncertainty: avg_uncertainty,
        tunneling_probability: tunneling_prob,
        energy_level,
    })
}

pub fn print_ascii_quantum(report: &QuantumReport) {
    println!("🌌 Market Quantum Analysis");
    println!("==============================");
    println!(
        "Uncertainty Principle (Δx*Δp): {:.2}",
        report.price_momentum_uncertainty
    );
    println!(
        "Tunneling Probability:         {:.2}%",
        report.tunneling_probability * 100.0
    );
    println!("Energy Level (n):              {}", report.energy_level);
    println!("==============================");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_test_bars() -> Vec<Bar> {
        vec![
            Bar {
                symbol: "TEST".into(),
                market: "crypto".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 1000,
                open: 100.0,
                high: 110.0,
                low: 90.0,
                close: 105.0,
                volume: 1000.0,
            },
            Bar {
                symbol: "TEST".into(),
                market: "crypto".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 2000,
                open: 115.0, // Gap up! (Tunneling)
                high: 120.0,
                low: 112.0,
                close: 118.0,
                volume: 1500.0,
            },
        ]
    }

    #[test]
    fn test_quantum_analysis() {
        let series = BarSeries {
            schema_version: "v1".into(),
            bars: create_test_bars(),
        };

        let report = analyze_quantum(&series, QuantumConfig::default()).unwrap();
        assert!(report.price_momentum_uncertainty > 0.0);
        assert_eq!(report.tunneling_probability, 1.0);
    }
}
