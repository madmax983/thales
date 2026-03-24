//! Fractal Dimension Analysis
//!
//! This module calculates the Higuchi Fractal Dimension (HFD) of a price series.
//! The fractal dimension measures the "roughness" or persistence of a time series:
//!
//! - **D ≈ 1.5**: Random Walk (Brownian motion). Price movements are independent.
//! - **D < 1.5**: Persistent (Trending). A positive move is likely followed by a positive move.
//! - **D > 1.5**: Anti-persistent (Mean-reverting). A positive move is likely followed by a negative move.
//!
//! # Core Concepts
//!
//! - **Higuchi Fractal Dimension (HFD)**: An algorithm to approximate the fractal dimension
//!   of a time series directly in the time domain.

use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Configuration for the Fractal Dimension analysis.
///
/// # Examples
///
/// ```rust
/// use thales_cli::fractal_dimension::FractalConfig;
///
/// let config = FractalConfig {
///     k_max: 10,
/// };
///
/// assert_eq!(config.k_max, 10);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FractalConfig {
    /// The maximum lag (k) to use in the Higuchi algorithm.
    /// Usually a small number like 5, 10, or 20 depending on the series length.
    pub k_max: usize,
}

/// The market regime derived from the fractal dimension.
///
/// # Examples
///
/// ```rust
/// use thales_cli::fractal_dimension::MarketRegime;
///
/// let regime = MarketRegime::Trending;
/// assert_eq!(format!("{}", regime), "Persistent (Trending)");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MarketRegime {
    Trending,
    RandomWalk,
    MeanReverting,
}

impl std::fmt::Display for MarketRegime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketRegime::Trending => write!(f, "Persistent (Trending)"),
            MarketRegime::RandomWalk => write!(f, "Random Walk"),
            MarketRegime::MeanReverting => write!(f, "Anti-persistent (Mean Reverting)"),
        }
    }
}

/// The result of a Fractal Dimension analysis.
///
/// # Examples
///
/// ```rust
/// use thales_cli::fractal_dimension::{FractalReport, MarketRegime};
///
/// let report = FractalReport {
///     symbol: "AAPL".to_string(),
///     dimension: 1.42,
///     regime: MarketRegime::Trending,
/// };
///
/// assert_eq!(report.dimension, 1.42);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FractalReport {
    /// The symbol analyzed.
    pub symbol: String,
    /// The calculated Higuchi Fractal Dimension (HFD).
    pub dimension: f64,
    /// The detected market regime based on the dimension.
    pub regime: MarketRegime,
}

/// Analyzes a [`BarSeries`] to calculate its Higuchi Fractal Dimension.
///
/// # Errors
///
/// Returns an error if the series has fewer points than `k_max + 1`.
///
/// # Examples
///
/// ```rust
/// use contracts::{Bar, BarSeries};
/// use thales_cli::fractal_dimension::{analyze_fractal_dimension, FractalConfig, MarketRegime};
///
/// // Create a simple upward trending series
/// let bars: Vec<Bar> = (0..20).map(|i| {
///     Bar {
///         symbol: "AAPL".to_string(),
///         market: "equities".to_string(),
///         timeframe: "1d".to_string(),
///         timestamp_unix_ms: i as i64 * 86400000,
///         open: 100.0 + i as f64,
///         high: 101.0 + i as f64,
///         low: 99.0 + i as f64,
///         close: 100.0 + i as f64,
///         volume: 1000.0,
///     }
/// }).collect();
///
/// let series = BarSeries {
///     schema_version: "v0".to_string(),
///     bars,
/// };
///
/// let config = FractalConfig { k_max: 5 };
/// let report = analyze_fractal_dimension(&series, config).unwrap();
///
/// assert_eq!(report.symbol, "AAPL");
/// // A straight line is highly persistent (trending)
/// assert_eq!(report.regime, MarketRegime::Trending);
/// ```
pub fn analyze_fractal_dimension(
    series: &BarSeries,
    config: FractalConfig,
) -> Result<FractalReport> {
    if series.bars.is_empty() {
        return Err(anyhow::anyhow!("Bar series cannot be empty"));
    }

    let n = series.bars.len();
    if n <= config.k_max {
        return Err(anyhow::anyhow!(
            "Series length ({}) must be strictly greater than k_max ({})",
            n,
            config.k_max
        ));
    }

    let prices: Vec<f64> = series.bars.iter().map(|b| b.close).collect();

    let dimension = higuchi_fd(&prices, config.k_max);

    let regime = if dimension < 1.45 {
        MarketRegime::Trending
    } else if dimension > 1.55 {
        MarketRegime::MeanReverting
    } else {
        MarketRegime::RandomWalk
    };

    Ok(FractalReport {
        symbol: series.bars[0].symbol.clone(),
        dimension,
        regime,
    })
}

/// Calculates the Higuchi Fractal Dimension of a 1D array.
fn higuchi_fd(data: &[f64], k_max: usize) -> f64 {
    let n = data.len();
    let mut ln_k = Vec::with_capacity(k_max);
    let mut ln_l = Vec::with_capacity(k_max);

    for k in 1..=k_max {
        let mut l_m = 0.0;
        for m in 1..=k {
            let mut sum = 0.0;
            let limit = (n - m) / k;
            for i in 1..=limit {
                sum += (data[m + i * k - 1] - data[m + (i - 1) * k - 1]).abs();
            }
            // Normalization factor
            let norm = ((n - 1) as f64) / ((limit * k) as f64);
            l_m += (sum * norm) / (k as f64);
        }
        let l_k = l_m / (k as f64);

        if l_k > 0.0 {
            ln_k.push((1.0 / (k as f64)).ln());
            ln_l.push(l_k.ln());
        }
    }

    if ln_k.is_empty() {
        return 1.5; // Fallback
    }

    // Linear regression of ln_L on ln_k to find the slope (dimension)
    // The slope of ln(L(k)) vs ln(1/k) is the dimension
    calculate_slope(&ln_k, &ln_l)
}

fn calculate_slope(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let sum_x: f64 = x.iter().sum();
    let sum_y: f64 = y.iter().sum();
    let sum_xy: f64 = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum();
    let sum_x_sq: f64 = x.iter().map(|a| a * a).sum();

    let denominator = n * sum_x_sq - sum_x.powi(2);
    if denominator == 0.0 {
        return 0.0;
    }

    (n * sum_xy - sum_x * sum_y) / denominator
}

pub fn print_ascii_fractal_dimension(report: &FractalReport) {
    println!("\nFractal Dimension Analysis: {}", report.symbol);
    println!("--------------------------------------------------");
    println!("Dimension (HFD):     {:.4}", report.dimension);

    let color = match report.regime {
        MarketRegime::Trending => "\x1b[1;32m",      // Green
        MarketRegime::RandomWalk => "\x1b[1;33m",    // Yellow
        MarketRegime::MeanReverting => "\x1b[1;31m", // Red
    };

    println!("Market Regime:       {}{}\x1b[0m", color, report.regime);
    println!("--------------------------------------------------\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_bar(symbol: &str, timestamp: i64, close: f64) -> Bar {
        Bar {
            symbol: symbol.to_string(),
            market: "equities".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: timestamp,
            open: close,
            high: close,
            low: close,
            close,
            volume: 100.0,
        }
    }

    #[test]
    fn test_trending_series() {
        // A perfectly straight line should have a dimension close to 1.0
        let mut bars = Vec::new();
        for i in 0..100 {
            bars.push(create_bar("TEST", i as i64 * 1000, i as f64));
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let config = FractalConfig { k_max: 10 };

        let report = analyze_fractal_dimension(&series, config).unwrap();

        assert!(
            report.dimension < 1.45,
            "Dimension should be < 1.45 for a straight line. Got {}",
            report.dimension
        );
        assert_eq!(report.regime, MarketRegime::Trending);
    }

    #[test]
    fn test_mean_reverting_series() {
        // A highly oscillating series (zigzag) should have a dimension close to 2.0
        let mut bars = Vec::new();
        for i in 0..100 {
            let close = if i % 2 == 0 { 10.0 } else { -10.0 };
            bars.push(create_bar("TEST", i as i64 * 1000, close));
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let config = FractalConfig { k_max: 10 };

        let report = analyze_fractal_dimension(&series, config).unwrap();

        assert!(
            report.dimension > 1.55,
            "Dimension should be > 1.55 for a zigzag line. Got {}",
            report.dimension
        );
        assert_eq!(report.regime, MarketRegime::MeanReverting);
    }
}
