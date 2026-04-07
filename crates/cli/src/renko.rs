//! # Renko Chart Analysis
//!
//! This module provides functionality for generating Renko charts from standard time-based price data.
//!
//! Renko charts (derived from the Japanese word "renga", meaning brick) are a type of financial chart
//! that measures price movement while ignoring time and volume. Unlike traditional candlesticks,
//! a new Renko "brick" is only drawn when the price moves a specific amount (the `brick_size`).
//!
//! ## Core Concepts
//! - **Noise Reduction:** By discarding minor price fluctuations and time, Renko charts make underlying trends much clearer.
//! - **Brick Size:** The foundational parameter. A larger brick size filters out more noise but reduces responsiveness.
//! - **Reversals:** It takes twice the `brick_size` to print a brick in the opposite direction of the previous trend. This helps prevent whipsaws.
//!
//! ## Usage
//! This module takes a standard `BarSeries` and outputs a `RenkoReport` containing the sequence
//! of generated bricks, which can then be analyzed for patterns or trend reversals.

use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Configuration for the Renko Chart analysis.
///
/// Defines the size of each brick in the Renko chart.
///
/// # Examples
///
/// ```
/// use thales_cli::renko::RenkoConfig;
///
/// let config = RenkoConfig {
///     brick_size: 1.0,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenkoConfig {
    /// The fixed size of each Renko brick.
    pub brick_size: f64,
}

/// Represents a single Renko brick.
///
/// A brick is formed only when the price moves by at least the `brick_size`
/// defined in the `RenkoConfig`.
///
/// # Examples
///
/// ```
/// use thales_cli::renko::RenkoBrick;
///
/// let brick = RenkoBrick {
///     bar_index: 0,
///     timestamp_unix_ms: 1600000000,
///     open: 10.0,
///     close: 11.0,
///     is_up: true,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenkoBrick {
    /// The index of the historical bar where this brick was completed.
    pub bar_index: usize,
    /// The timestamp of the historical bar where this brick was completed.
    pub timestamp_unix_ms: i64,
    /// The opening price of this brick.
    pub open: f64,
    /// The closing price of this brick.
    pub close: f64,
    /// The direction of the brick (true = up/green, false = down/red).
    pub is_up: bool,
}

/// The result of a Renko analysis run.
///
/// Contains the full sequence of generated bricks, along with summary statistics.
///
/// # Examples
///
/// ```
/// use thales_cli::renko::{RenkoReport, RenkoBrick};
///
/// let report = RenkoReport {
///     symbol: "BTC".to_string(),
///     bricks: vec![
///         RenkoBrick { bar_index: 0, timestamp_unix_ms: 1600000000, open: 10.0, close: 11.0, is_up: true },
///     ],
///     total_up_bricks: 1,
///     total_down_bricks: 0,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenkoReport {
    /// The symbol analyzed.
    pub symbol: String,
    /// The sequence of computed Renko bricks.
    pub bricks: Vec<RenkoBrick>,
    /// The total number of up bricks.
    pub total_up_bricks: usize,
    /// The total number of down bricks.
    pub total_down_bricks: usize,
}

/// Analyzes a [`BarSeries`] to compute a Renko chart.
///
/// Renko charts filter out time and minor price movements, focusing only on
/// significant price changes defined by `brick_size`. A new brick is formed
/// only when the price moves more than `brick_size` from the previous brick's close.
///
/// # Errors
///
/// Returns an error if:
/// - The `BarSeries` is empty.
/// - `config.brick_size` is <= 0.
///
/// # Examples
///
/// ```rust
/// use contracts::{BarSeries, Bar};
/// use thales_cli::renko::{analyze_renko, RenkoConfig};
///
/// fn create_bar(timestamp: i64, close: f64) -> Bar {
///     Bar {
///         symbol: "TEST".to_string(),
///         market: "equities".to_string(),
///         timeframe: "1d".to_string(),
///         timestamp_unix_ms: timestamp,
///         open: close,
///         high: close,
///         low: close,
///         close,
///         volume: 100.0,
///     }
/// }
///
/// let bars = vec![
///     create_bar(1000, 10.0), // Start
///     create_bar(2000, 11.0), // +1 (New Up Brick)
///     create_bar(3000, 11.5), // +0.5 (No brick)
///     create_bar(4000, 13.0), // +1.5 (Two Up Bricks: 12.0, 13.0)
///     create_bar(5000, 11.0), // -2 (Reversal requires 2x brick size from close, so one down brick to 12.0, then one to 11.0)
/// ];
///
/// let series = BarSeries { schema_version: "v0".to_string(), bars };
/// let config = RenkoConfig { brick_size: 1.0 };
///
/// let report = analyze_renko(&series, config).unwrap();
/// assert_eq!(report.bricks.len(), 4);
/// ```
pub fn analyze_renko(series: &BarSeries, config: RenkoConfig) -> Result<RenkoReport> {
    if series.bars.is_empty() {
        return Err(anyhow::anyhow!("Bar series cannot be empty"));
    }

    if config.brick_size <= 0.0 {
        return Err(anyhow::anyhow!("brick_size must be > 0"));
    }

    let symbol = series.bars[0].symbol.clone();
    let mut bricks = Vec::new();

    // The reference price for the next brick calculation
    let mut current_reference_price = series.bars[0].close;

    // Track the direction of the last brick (None at start)
    let mut last_brick_is_up: Option<bool> = None;

    for (i, bar) in series.bars.iter().enumerate() {
        let price = bar.close;

        let diff = price - current_reference_price;
        let mut num_bricks = (diff.abs() / config.brick_size).floor() as usize;

        if num_bricks > 0 {
            let is_up = diff > 0.0;

            // Handle reversal requirement (price must move 2x brick size in opposite direction)
            if last_brick_is_up.is_some() && is_up != last_brick_is_up.unwrap() {
                // It's a reversal. The first "brick size" of movement just covers the body of the previous brick.
                // We only form actual reversal bricks if the movement is >= 2x brick size.
                if num_bricks >= 2 {
                    // The first 'brick_size' movement cancels out the previous brick's body to start the reversal.
                    num_bricks -= 1;
                    // The reference price effectively shifts to the *open* of the previous brick before stepping.
                    if is_up {
                        current_reference_price += config.brick_size;
                    } else {
                        current_reference_price -= config.brick_size;
                    }
                } else {
                    // Not enough movement for a reversal brick
                    continue;
                }
            }

            for _ in 0..num_bricks {
                let (brick_open, brick_close) = if is_up {
                    (
                        current_reference_price,
                        current_reference_price + config.brick_size,
                    )
                } else {
                    (
                        current_reference_price,
                        current_reference_price - config.brick_size,
                    )
                };

                bricks.push(RenkoBrick {
                    bar_index: i,
                    timestamp_unix_ms: bar.timestamp_unix_ms,
                    open: brick_open,
                    close: brick_close,
                    is_up,
                });

                current_reference_price = brick_close;
            }

            last_brick_is_up = Some(is_up);
        }
    }

    let total_up_bricks = bricks.iter().filter(|b| b.is_up).count();
    let total_down_bricks = bricks.len() - total_up_bricks;

    Ok(RenkoReport {
        symbol,
        bricks,
        total_up_bricks,
        total_down_bricks,
    })
}

#[cfg(feature = "nova")]
pub fn print_ascii_renko(report: &RenkoReport) {
    println!("\nRenko Chart Analysis for {}", report.symbol);
    println!("--------------------------------------------------");
    println!("Total Bricks: {}", report.bricks.len());
    println!("Up Bricks:    {}", report.total_up_bricks);
    println!("Down Bricks:  {}", report.total_down_bricks);

    if report.bricks.is_empty() {
        println!("No bricks formed.");
        return;
    }

    // Determine min/max for scaling (simple textual representation)
    let max_price = report
        .bricks
        .iter()
        .map(|b| b.open.max(b.close))
        .fold(f64::MIN, |a, b| a.max(b));
    let min_price = report
        .bricks
        .iter()
        .map(|b| b.open.min(b.close))
        .fold(f64::MAX, |a, b| a.min(b));

    println!("Price Range:  {:.2} to {:.2}", min_price, max_price);
    println!("--------------------------------------------------");

    // Print a simple horizontal sequence of the last 50 bricks
    let max_display = 50;
    let display_bricks = if report.bricks.len() > max_display {
        &report.bricks[report.bricks.len() - max_display..]
    } else {
        &report.bricks[..]
    };

    print!("Chart (latest {}): ", display_bricks.len());
    for brick in display_bricks {
        if brick.is_up {
            print!("\x1b[1;32m[]\x1b[0m"); // Green []
        } else {
            print!("\x1b[1;31m[]\x1b[0m"); // Red []
        }
    }
    println!("\n--------------------------------------------------\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_bar(timestamp: i64, close: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
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
    fn test_renko_basic_uptrend() {
        let bars = vec![
            create_bar(1000, 10.0), // Base
            create_bar(2000, 11.0), // +1 brick
            create_bar(3000, 12.0), // +1 brick
            create_bar(4000, 12.5), // No brick
            create_bar(5000, 13.0), // +1 brick
        ];
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let config = RenkoConfig { brick_size: 1.0 };
        let report = analyze_renko(&series, config).unwrap();

        assert_eq!(report.bricks.len(), 3);
        assert!(report.bricks.iter().all(|b| b.is_up));
        assert_eq!(report.bricks[0].close, 11.0);
        assert_eq!(report.bricks[1].close, 12.0);
        assert_eq!(report.bricks[2].close, 13.0);
    }

    #[test]
    fn test_renko_reversal() {
        let bars = vec![
            create_bar(1000, 10.0), // Base
            create_bar(2000, 11.0), // Up to 11
            create_bar(3000, 10.0), // Down 1 -> Reversal needs 2! So no brick yet.
            create_bar(4000, 9.0), // Down another 1 -> Total 2 down from 11.0 close. Reversal brick formed!
        ];
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let config = RenkoConfig { brick_size: 1.0 };
        let report = analyze_renko(&series, config).unwrap();

        assert_eq!(report.bricks.len(), 2);
        assert!(report.bricks[0].is_up);
        assert_eq!(report.bricks[0].close, 11.0);

        assert!(!report.bricks[1].is_up);
        assert_eq!(report.bricks[1].close, 9.0); // Open at 10.0, close at 9.0
    }

    #[test]
    fn test_renko_multiple_bricks_one_bar() {
        let bars = vec![
            create_bar(1000, 10.0), // Base
            create_bar(2000, 13.0), // Jumps 3 sizes -> 3 up bricks
        ];
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let config = RenkoConfig { brick_size: 1.0 };
        let report = analyze_renko(&series, config).unwrap();

        assert_eq!(report.bricks.len(), 3);
        assert_eq!(report.bricks[0].close, 11.0);
        assert_eq!(report.bricks[1].close, 12.0);
        assert_eq!(report.bricks[2].close, 13.0);
    }
}
