//! Fractal Pattern Matching Analysis
//!
//! This module provides tools for identifying historical price patterns that closely resemble
//! the current market price action. By finding similar historical setups, it estimates the
//! expected forward return based on what happened next in the past.
//!
//! # Core Concepts
//!
//! - **Target Window:** The most recent sequence of bars (length `window_size`) acting as the reference pattern.
//! - **Distance:** The Euclidean distance between the normalized target window and a historical window.
//! - **Similarity Score:** A value between 0.0 and 1.0 indicating how closely the historical pattern matches
//!   the target window (1.0 means identical).
//! - **Forward Return:** The percentage price change in the `forward_horizon` periods immediately following
//!   a matched historical pattern.

use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Configuration for the Pattern Matching analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatchConfig {
    /// The number of recent bars to use as the target pattern.
    pub window_size: usize,
    /// The maximum number of historical matches to return.
    pub top_k: usize,
    /// The number of periods ahead to calculate the expected return for each match.
    pub forward_horizon: usize,
}

/// The result of a Pattern Matching analysis run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatchReport {
    /// The asset symbol analyzed.
    pub symbol: String,
    /// The list of the top `k` historical pattern matches found.
    pub matches: Vec<PatternMatch>,
    /// The average expected forward return percentage based on the top matches.
    pub expected_forward_return_pct: f64,
}

/// A single historical pattern match found by the analyzer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatch {
    /// The starting index of the matched pattern in the historical data.
    pub start_index: usize,
    /// The ending index of the matched pattern in the historical data.
    pub end_index: usize,
    /// The Euclidean distance between the normalized historical and target patterns.
    pub distance: f64,
    /// A score from 0.0 to 1.0 indicating similarity (1.0 is a perfect match).
    pub similarity_score: f64,
    /// The actual percentage return observed `forward_horizon` periods after this historical pattern.
    pub forward_return_pct: f64,
}

/// Analyzes a [`BarSeries`] to find historical patterns that match the most recent price action.
///
/// The function takes the last `window_size` bars as the "target pattern". It normalizes this
/// pattern (scales values between 0.0 and 1.0) and then slides a window of the same size over
/// the entire historical dataset.
///
/// For each historical window, it normalizes the prices, calculates the Euclidean distance to
/// the target pattern, and determines what the actual return was `forward_horizon` bars later.
/// It returns the top `top_k` matches with the lowest Euclidean distance.
///
/// # Errors
///
/// Returns an error if:
/// - The `BarSeries` has fewer bars than `window_size + forward_horizon`.
/// - `config.window_size` or `config.top_k` is 0.
///
/// # Examples
///
/// ```rust
/// use contracts::{BarSeries, Bar};
/// use thales_cli::pattern_match::{analyze_patterns, PatternMatchConfig};
///
/// fn create_bar(close: f64) -> Bar {
///     Bar {
///         symbol: "TEST".to_string(),
///         market: "equities".to_string(),
///         timeframe: "1d".to_string(),
///         timestamp_unix_ms: 100000,
///         open: 100.0,
///         high: 100.0,
///         low: 100.0,
///         close,
///         volume: 100.0,
///     }
/// }
///
/// // Create a simple V-shape pattern historically, followed by a spike
/// let bars = vec![
///     create_bar(10.0),  // Hist start
///     create_bar(5.0),   // Hist mid
///     create_bar(10.0),  // Hist end
///     create_bar(15.0),  // Forward horizon (+50% return)
///     create_bar(100.0), // Noise
///     create_bar(200.0), // Noise
///     create_bar(10.0),  // Target start
///     create_bar(5.0),   // Target mid
///     create_bar(10.0),  // Target end
/// ];
///
/// let series = BarSeries { schema_version: "v0".to_string(), bars };
///
/// let config = PatternMatchConfig {
///     window_size: 3,
///     top_k: 1,
///     forward_horizon: 1,
/// };
///
/// let report = analyze_patterns(&series, config).unwrap();
/// assert_eq!(report.matches.len(), 1);
/// assert_eq!(report.matches[0].start_index, 0);
/// assert!((report.expected_forward_return_pct - 50.0).abs() < f64::EPSILON);
/// ```
pub fn analyze_patterns(
    series: &BarSeries,
    config: PatternMatchConfig,
) -> Result<PatternMatchReport> {
    if series.bars.len() < config.window_size + config.forward_horizon {
        return Err(anyhow::anyhow!(
            "Not enough bars to match patterns. Need at least {} bars.",
            config.window_size + config.forward_horizon
        ));
    }

    if config.window_size == 0 || config.top_k == 0 {
        return Err(anyhow::anyhow!("window_size and top_k must be > 0"));
    }

    let symbol = series.bars[0].symbol.clone();
    let num_bars = series.bars.len();

    // Extract the target window (most recent `window_size` bars)
    let target_bars = &series.bars[num_bars - config.window_size..];
    let target_close = target_bars.iter().map(|b| b.close).collect::<Vec<_>>();
    let target_normalized = normalize(&target_close);

    let mut matches = Vec::new();

    // Slide window over historical data
    // Stop early enough to allow for `forward_horizon` and avoid overlapping completely with the target
    let max_start_idx = num_bars - config.window_size - config.forward_horizon;

    // We also avoid overlapping the reference with the target itself to some degree
    // But for simplicity, we just slide up to max_start_idx.

    for i in 0..=max_start_idx {
        let hist_window = &series.bars[i..i + config.window_size];
        let hist_close = hist_window.iter().map(|b| b.close).collect::<Vec<_>>();

        // Skip flat lines to avoid divide-by-zero or weird normalization
        if hist_close
            .iter()
            .all(|&x| (x - hist_close[0]).abs() < f64::EPSILON)
        {
            continue;
        }

        let hist_normalized = normalize(&hist_close);

        let distance = euclidean_distance(&target_normalized, &hist_normalized);

        // Calculate forward return
        let current_price = hist_window.last().unwrap().close;
        let future_price = series.bars[i + config.window_size + config.forward_horizon - 1].close;

        let forward_return_pct = if current_price > 0.0 {
            (future_price - current_price) / current_price * 100.0
        } else {
            0.0
        };

        matches.push(PatternMatch {
            start_index: i,
            end_index: i + config.window_size - 1,
            distance,
            // Simple heuristic to bound similarity between 0 and 1
            similarity_score: 1.0 / (1.0 + distance),
            forward_return_pct,
        });
    }

    // Sort by distance ascending (most similar first)
    matches.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());

    // Take top K
    matches.truncate(config.top_k);

    let expected_return = if matches.is_empty() {
        0.0
    } else {
        matches.iter().map(|m| m.forward_return_pct).sum::<f64>() / matches.len() as f64
    };

    Ok(PatternMatchReport {
        symbol,
        matches,
        expected_forward_return_pct: expected_return,
    })
}

fn normalize(data: &[f64]) -> Vec<f64> {
    if data.is_empty() {
        return vec![];
    }
    let min = data.iter().fold(f64::MAX, |a, &b| a.min(b));
    let max = data.iter().fold(f64::MIN, |a, &b| a.max(b));

    if (max - min).abs() < f64::EPSILON {
        return vec![0.0; data.len()]; // Flat line
    }

    data.iter().map(|&x| (x - min) / (max - min)).collect()
}

fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

#[cfg(feature = "nova")]
pub fn print_ascii_patterns(report: &PatternMatchReport) {
    println!("\nFractal Pattern Matcher Report for {}", report.symbol);
    println!("--------------------------------------------------");

    if report.matches.is_empty() {
        println!("No historical patterns found.");
        return;
    }

    println!("Top {} Matches Found:", report.matches.len());
    println!(
        "{:>5} | {:>8} | {:>10} | {:>10} | {:>15}",
        "Rank", "Sim %", "Distance", "Fwd Ret %", "Indices (Start-End)"
    );
    println!("----------------------------------------------------------------------");

    for (i, m) in report.matches.iter().enumerate() {
        println!(
            "{:>5} | {:>7.2}% | {:>10.4} | {:>9.2}% | {:>7}-{:<7}",
            i + 1,
            m.similarity_score * 100.0,
            m.distance,
            m.forward_return_pct,
            m.start_index,
            m.end_index
        );
    }

    println!("----------------------------------------------------------------------");

    let color_start = if report.expected_forward_return_pct > 0.0 {
        "\x1b[1;32m" // Green
    } else if report.expected_forward_return_pct < 0.0 {
        "\x1b[1;31m" // Red
    } else {
        "\x1b[1;33m" // Yellow
    };
    let color_end = "\x1b[0m";

    println!(
        "Expected Forward Return: {}{:.2}%{}",
        color_start, report.expected_forward_return_pct, color_end
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_bar(close: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 100000,
            open: 100.0,
            high: 100.0,
            low: 100.0,
            close,
            volume: 100.0,
        }
    }

    #[test]
    fn test_pattern_match_exact() {
        // A simple "V" shape pattern
        // 10, 5, 10
        let bars = vec![
            create_bar(10.0), // Hist start
            create_bar(5.0),
            create_bar(10.0),
            create_bar(15.0),  // Fwd ret: (15-10)/10 = 50%
            create_bar(100.0), // Noise
            create_bar(200.0), // Noise
            create_bar(10.0),  // Target start
            create_bar(5.0),
            create_bar(10.0), // Target end (will ask for 1 fwd horizon, so we need 1 more bar? No, target is at the end, fwd horizon is for historical matches)
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = PatternMatchConfig {
            window_size: 3,
            top_k: 1,
            forward_horizon: 1,
        };

        let report = analyze_patterns(&series, config).unwrap();

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].start_index, 0);
        assert_eq!(report.matches[0].end_index, 2);
        assert!((report.matches[0].distance - 0.0).abs() < f64::EPSILON);
        assert!((report.matches[0].forward_return_pct - 50.0).abs() < f64::EPSILON);
        assert!((report.expected_forward_return_pct - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pattern_match_normalized() {
        // A simple "V" shape pattern, scaled
        let bars = vec![
            create_bar(100.0), // Hist start
            create_bar(50.0),
            create_bar(100.0),
            create_bar(110.0), // Fwd ret: (110-100)/100 = 10%
            create_bar(100.0), // Noise
            create_bar(200.0), // Noise
            create_bar(10.0),  // Target start
            create_bar(5.0),
            create_bar(10.0), // Target end
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = PatternMatchConfig {
            window_size: 3,
            top_k: 1,
            forward_horizon: 1,
        };

        let report = analyze_patterns(&series, config).unwrap();

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].start_index, 0);
        // Distance should be 0 because after normalization [100,50,100] -> [1.0, 0.0, 1.0] and [10,5,10] -> [1.0, 0.0, 1.0]
        assert!((report.matches[0].distance - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pattern_match_not_enough_bars() {
        let bars = vec![create_bar(10.0), create_bar(5.0)];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = PatternMatchConfig {
            window_size: 3,
            top_k: 1,
            forward_horizon: 1,
        };

        let result = analyze_patterns(&series, config);
        assert!(result.is_err());
    }
}
