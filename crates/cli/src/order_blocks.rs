//! Order Blocks Analysis
//!
//! This module implements Smart Money Concepts (SMC) order block detection.
//! Order blocks are specific price levels where institutional market participants
//! are believed to have placed large orders.
//!
//! # Core Concepts
//!
//! - **Order Block (OB)**: Typically the last opposite-colored candle before a strong move.
//!   - *Bullish OB*: The last bearish candle before a strong upward expansion.
//!   - *Bearish OB*: The last bullish candle before a strong downward expansion.
//! - **Expansion**: A price move significantly larger than the recent average true range (ATR).
//! - **Mitigation**: When price later returns to the order block level and touches it, the
//!   orders are considered filled (mitigated). Unmitigated order blocks act as strong future support/resistance.

use anyhow::Result;
#[cfg(test)]
use contracts::Bar;
use contracts::BarSeries;
use serde::Serialize;

/// Configuration for Order Blocks analysis.
///
/// # Examples
///
/// ```rust
/// use thales_cli::order_blocks::OrderBlocksConfig;
///
/// let config = OrderBlocksConfig {
///     atr_period: 14,
///     expansion_multiplier: 1.5,
/// };
///
/// assert_eq!(config.atr_period, 14);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct OrderBlocksConfig {
    /// The number of previous bars used to calculate average range.
    pub atr_period: usize,
    /// Multiplier to determine if a bar is a "strong move" compared to average range.
    pub expansion_multiplier: f64,
}

/// Represents a detected Order Block.
///
/// # Examples
///
/// ```rust
/// use thales_cli::order_blocks::OrderBlock;
///
/// let block = OrderBlock {
///     is_bullish: true,
///     index: 5,
///     timestamp_ms: 1622505600000,
///     top: 105.0,
///     bottom: 100.0,
///     mitigation_index: None,
/// };
///
/// assert!(block.is_bullish);
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct OrderBlock {
    /// True if bullish (demand zone), false if bearish (supply zone).
    pub is_bullish: bool,
    /// The index of the bar in the series where this order block occurred.
    pub index: usize,
    pub timestamp_ms: i64,
    pub top: f64,
    pub bottom: f64,
    /// The index of the bar that mitigated this block, if any.
    pub mitigation_index: Option<usize>,
}

/// The result of an Order Blocks analysis run.
///
/// # Examples
///
/// ```rust
/// use thales_cli::order_blocks::{OrderBlocksReport, OrderBlock};
///
/// let report = OrderBlocksReport {
///     blocks: vec![],
/// };
///
/// assert!(report.blocks.is_empty());
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct OrderBlocksReport {
    /// All detected order blocks (both mitigated and unmitigated).
    pub blocks: Vec<OrderBlock>,
}

/// Analyzes a [`BarSeries`] to detect Smart Money Concepts (SMC) order blocks.
///
/// Returns a report containing both mitigated and unmitigated order blocks.
///
/// # Examples
///
/// ```rust
/// use contracts::{Bar, BarSeries};
/// use thales_cli::order_blocks::{analyze_order_blocks, OrderBlocksConfig};
///
/// // Create mock series
/// let bars: Vec<Bar> = (0..10).map(|i| {
///     Bar {
///         symbol: "AAPL".to_string(),
///         market: "equities".to_string(),
///         timeframe: "1d".to_string(),
///         timestamp_unix_ms: i as i64 * 86400000,
///         open: 100.0,
///         high: 101.0,
///         low: 99.0,
///         close: 100.0,
///         volume: 1000.0,
///     }
/// }).collect();
///
/// let series = BarSeries {
///     schema_version: "v0".to_string(),
///     bars,
/// };
///
/// let config = OrderBlocksConfig {
///     atr_period: 3,
///     expansion_multiplier: 2.0,
/// };
///
/// let report = analyze_order_blocks(&series, config).unwrap();
/// assert!(report.blocks.is_empty()); // No strong expansions in this flat data
/// ```
pub fn analyze_order_blocks(
    series: &BarSeries,
    config: OrderBlocksConfig,
) -> Result<OrderBlocksReport> {
    if series.bars.len() < config.atr_period + 2 {
        return Ok(OrderBlocksReport { blocks: vec![] });
    }

    let mut blocks = Vec::new();
    let mut tr_sum = 0.0;

    // Calculate initial average true range approximation (just simple range for now)
    for i in 1..=config.atr_period {
        tr_sum += series.bars[i].high - series.bars[i].low;
    }

    let mut avg_range = tr_sum / config.atr_period as f64;

    for i in (config.atr_period + 1)..series.bars.len() {
        let current = &series.bars[i];
        let previous = &series.bars[i - 1];

        let current_range = current.high - current.low;
        let is_strong_move = current_range > (avg_range * config.expansion_multiplier);

        if is_strong_move {
            let is_bullish_move = current.close > current.open;

            // Look for the opposite candle before the strong move
            if is_bullish_move && previous.close < previous.open {
                // Bullish Order Block (last bearish candle before strong bullish move)
                blocks.push(OrderBlock {
                    is_bullish: true,
                    index: i - 1,
                    timestamp_ms: previous.timestamp_unix_ms,
                    top: previous.high,
                    bottom: previous.low,
                    mitigation_index: None,
                });
            } else if !is_bullish_move && previous.close > previous.open {
                // Bearish Order Block (last bullish candle before strong bearish move)
                blocks.push(OrderBlock {
                    is_bullish: false,
                    index: i - 1,
                    timestamp_ms: previous.timestamp_unix_ms,
                    top: previous.high,
                    bottom: previous.low,
                    mitigation_index: None,
                });
            }
        }

        // Update avg range (simple EMA-like approximation)
        avg_range = (avg_range * (config.atr_period as f64 - 1.0) + current_range)
            / config.atr_period as f64;
    }

    // Simple mitigation check
    for block in &mut blocks {
        if block.mitigation_index.is_none() {
            let start_idx = block.index + 2; // start checking after the expansion candle
            for (k, check_bar) in series.bars.iter().enumerate().skip(start_idx) {
                let is_mitigated = if block.is_bullish {
                    check_bar.low <= block.top // Price came down and touched/pierced the OB top
                } else {
                    check_bar.high >= block.bottom // Price went up and touched/pierced the OB bottom
                };

                if is_mitigated {
                    block.mitigation_index = Some(k);
                    break;
                }
            }
        }
    }

    Ok(OrderBlocksReport { blocks })
}

pub fn print_ascii_order_blocks(report: &OrderBlocksReport) {
    println!("\n=== SMC Order Blocks (Unmitigated) ===");
    let mut active = 0;

    for block in &report.blocks {
        if block.mitigation_index.is_none() {
            active += 1;
            let dir = if block.is_bullish {
                "BULLISH (Demand)"
            } else {
                "BEARISH (Supply)"
            };
            println!(
                "[{}] {} OB at {}-{}",
                block.index, dir, block.bottom, block.top
            );
        }
    }

    if active == 0 {
        println!("No unmitigated order blocks found.");
    }
    println!("======================================");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_bars() -> Vec<Bar> {
        vec![
            Bar {
                symbol: "BTC".into(),
                market: "crypto".into(),
                timeframe: "1h".into(),
                timestamp_unix_ms: 0,
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 100.0,
                volume: 1.0,
            },
            Bar {
                symbol: "BTC".into(),
                market: "crypto".into(),
                timeframe: "1h".into(),
                timestamp_unix_ms: 1,
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 100.0,
                volume: 1.0,
            },
            Bar {
                symbol: "BTC".into(),
                market: "crypto".into(),
                timeframe: "1h".into(),
                timestamp_unix_ms: 2,
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 100.0,
                volume: 1.0,
            },
            Bar {
                symbol: "BTC".into(),
                market: "crypto".into(),
                timeframe: "1h".into(),
                timestamp_unix_ms: 3,
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 100.0,
                volume: 1.0,
            },
            // Small bearish candle (The potential OB)
            Bar {
                symbol: "BTC".into(),
                market: "crypto".into(),
                timeframe: "1h".into(),
                timestamp_unix_ms: 4,
                open: 100.0,
                high: 102.0,
                low: 98.0,
                close: 99.0,
                volume: 1.0,
            },
            // Strong bullish expansion
            Bar {
                symbol: "BTC".into(),
                market: "crypto".into(),
                timeframe: "1h".into(),
                timestamp_unix_ms: 5,
                open: 100.0,
                high: 130.0,
                low: 99.0,
                close: 125.0,
                volume: 5.0,
            },
            // Next candles
            Bar {
                symbol: "BTC".into(),
                market: "crypto".into(),
                timeframe: "1h".into(),
                timestamp_unix_ms: 6,
                open: 125.0,
                high: 130.0,
                low: 120.0,
                close: 128.0,
                volume: 1.0,
            },
            Bar {
                symbol: "BTC".into(),
                market: "crypto".into(),
                timeframe: "1h".into(),
                timestamp_unix_ms: 7,
                open: 128.0,
                high: 135.0,
                low: 125.0,
                close: 130.0,
                volume: 1.0,
            },
            // Mitigation candle (touches the OB top which is 102.0)
            Bar {
                symbol: "BTC".into(),
                market: "crypto".into(),
                timeframe: "1h".into(),
                timestamp_unix_ms: 8,
                open: 130.0,
                high: 135.0,
                low: 100.0,
                close: 110.0,
                volume: 2.0,
            },
        ]
    }

    #[test]
    fn test_order_blocks() -> anyhow::Result<()> {
        let series = BarSeries {
            schema_version: "v1".into(),
            bars: create_test_bars(),
        };

        let config = OrderBlocksConfig {
            atr_period: 3,
            expansion_multiplier: 2.0,
        };

        let report = analyze_order_blocks(&series, config)?;

        let ob = report
            .blocks
            .iter()
            .find(|b| b.index == 4)
            .ok_or_else(|| anyhow::anyhow!("Expected block at index 4 not found"))?;
        assert!(ob.is_bullish);
        assert_eq!(ob.mitigation_index, Some(8)); // Touched at index 8
        Ok(())
    }
}
