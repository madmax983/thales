//! # Candlestick Pattern Recognition Module
//!
//! This module provides algorithms for detecting classic candlestick patterns
//! (like Doji, Hammer, Shooting Star, and Engulfing patterns) within a [`BarSeries`].
//!
//! It helps traders analyze market sentiment and identify potential reversals.
//!
//! ## Core Concepts
//! - **Doji**: A neutral pattern indicating indecision.
//! - **Hammer**: A bullish reversal pattern formed after a decline.
//! - **Shooting Star**: A bearish reversal pattern formed after an advance.
//! - **Engulfing**: Strong reversal patterns (bullish or bearish).

use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Configuration for Candlestick Pattern detection.
///
/// ## Examples
/// ```
/// use thales_cli::experimental::candlestick_patterns::CandlestickPatternsConfig;
///
/// let config = CandlestickPatternsConfig {
///     window_size: 14,
///     doji_threshold_pct: 0.15,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandlestickPatternsConfig {
    pub window_size: usize,
    pub doji_threshold_pct: f64,
}

impl Default for CandlestickPatternsConfig {
    fn default() -> Self {
        Self {
            window_size: 10,
            doji_threshold_pct: 0.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatch {
    pub pattern_name: String,
    pub timestamp_unix_ms: i64,
    pub bullish: bool,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandlestickPatternsReport {
    pub symbol: String,
    pub config: CandlestickPatternsConfig,
    pub matches: Vec<PatternMatch>,
    pub overall_sentiment: String,
    pub total_bullish: usize,
    pub total_bearish: usize,
}

/// Analyzes a `BarSeries` for classic candlestick patterns.
///
/// ## Errors
/// Returns an `Err` if the `BarSeries` is empty.
///
/// ## Examples
/// ```
/// use thales_cli::experimental::candlestick_patterns::{analyze_candlestick_patterns, CandlestickPatternsConfig};
/// use contracts::{BarSeries, Bar};
///
/// let mut series = BarSeries { schema_version: "v0".to_string(), bars: vec![] };
/// series.bars.push(Bar {
///     symbol: "AAPL".to_string(),
///     market: "equities".to_string(),
///     timeframe: "1d".to_string(),
///     timestamp_unix_ms: 1000,
///     open: 100.0,
///     high: 110.0,
///     low: 90.0,
///     close: 95.0,
///     volume: 1000.0,
/// });
/// series.bars.push(Bar {
///     symbol: "AAPL".to_string(),
///     market: "equities".to_string(),
///     timeframe: "1d".to_string(),
///     timestamp_unix_ms: 2000,
///     open: 100.0,
///     high: 105.0,
///     low: 95.0,
///     close: 100.001,
///     volume: 1000.0,
/// });
///
/// let config = CandlestickPatternsConfig { window_size: 2, doji_threshold_pct: 0.1 };
/// let report = analyze_candlestick_patterns(&series, config).unwrap();
/// assert_eq!(report.matches.len(), 1);
/// assert_eq!(report.matches[0].pattern_name, "Doji");
/// ```
pub fn analyze_candlestick_patterns(
    series: &BarSeries,
    config: CandlestickPatternsConfig,
) -> Result<CandlestickPatternsReport> {
    if series.bars.is_empty() {
        return Err(anyhow::anyhow!("Empty BarSeries provided"));
    }

    let mut matches = Vec::new();
    let window_size = config.window_size;
    let doji_threshold_pct = config.doji_threshold_pct;

    // Analyze the last `window_size` bars, but we also need previous bars for some patterns like Engulfing.
    let n = series.bars.len();
    let start_idx = if n > window_size { n - window_size } else { 1 };

    for i in start_idx..n {
        let bar = &series.bars[i];
        let prev_bar = &series.bars[i - 1];

        let body_size = (bar.close - bar.open).abs();
        let total_size = bar.high - bar.low;
        let is_green = bar.close > bar.open;
        let is_red = bar.close < bar.open;

        let is_prev_green = prev_bar.close > prev_bar.open;
        let is_prev_red = prev_bar.close < prev_bar.open;

        let upper_shadow = bar.high - bar.close.max(bar.open);
        let lower_shadow = bar.close.min(bar.open) - bar.low;

        // 1. Doji
        if total_size > 0.0 && (body_size / total_size) <= (doji_threshold_pct / 100.0) {
            matches.push(PatternMatch {
                pattern_name: "Doji".to_string(),
                timestamp_unix_ms: bar.timestamp_unix_ms,
                bullish: false, // Doji is neutral, but we'll mark it based on context or just false
                confidence: 0.5,
            });
        }

        // 2. Hammer
        // Small body, long lower shadow, little to no upper shadow.
        if total_size > 0.0 && lower_shadow > body_size * 2.0 && upper_shadow < body_size * 0.2 {
            matches.push(PatternMatch {
                pattern_name: "Hammer".to_string(),
                timestamp_unix_ms: bar.timestamp_unix_ms,
                bullish: true,
                confidence: 0.7,
            });
        }

        // 3. Shooting Star
        // Small body, long upper shadow, little to no lower shadow.
        if total_size > 0.0 && upper_shadow > body_size * 2.0 && lower_shadow < body_size * 0.2 {
            matches.push(PatternMatch {
                pattern_name: "Shooting Star".to_string(),
                timestamp_unix_ms: bar.timestamp_unix_ms,
                bullish: false,
                confidence: 0.7,
            });
        }

        // 4. Bullish Engulfing
        if is_prev_red && is_green && bar.close > prev_bar.open && bar.open < prev_bar.close {
            matches.push(PatternMatch {
                pattern_name: "Bullish Engulfing".to_string(),
                timestamp_unix_ms: bar.timestamp_unix_ms,
                bullish: true,
                confidence: 0.8,
            });
        }

        // 5. Bearish Engulfing
        if is_prev_green && is_red && bar.open > prev_bar.close && bar.close < prev_bar.open {
            matches.push(PatternMatch {
                pattern_name: "Bearish Engulfing".to_string(),
                timestamp_unix_ms: bar.timestamp_unix_ms,
                bullish: false,
                confidence: 0.8,
            });
        }
    }

    let mut total_bullish = 0;
    let mut total_bearish = 0;
    for m in &matches {
        if m.bullish && m.pattern_name != "Doji" {
            total_bullish += 1;
        } else if !m.bullish && m.pattern_name != "Doji" {
            total_bearish += 1;
        }
    }

    let overall_sentiment = if total_bullish > total_bearish {
        "Bullish".to_string()
    } else if total_bearish > total_bullish {
        "Bearish".to_string()
    } else {
        "Neutral".to_string()
    };

    Ok(CandlestickPatternsReport {
        symbol: series
            .bars
            .first()
            .map(|b| b.symbol.clone())
            .unwrap_or_default(),
        config,
        matches,
        overall_sentiment,
        total_bullish,
        total_bearish,
    })
}

pub fn print_ascii_patterns(report: &CandlestickPatternsReport) {
    println!("=== Candlestick Patterns Report ===");
    println!("Symbol: {}", report.symbol);
    println!("Overall Sentiment: {}", report.overall_sentiment);
    println!("Bullish Patterns: {}", report.total_bullish);
    println!("Bearish Patterns: {}", report.total_bearish);
    println!("Recent Pattern Matches:");
    for m in &report.matches {
        let dt = chrono::DateTime::from_timestamp_millis(m.timestamp_unix_ms)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let sentiment = if m.pattern_name == "Doji" {
            "Neutral"
        } else if m.bullish {
            "Bullish"
        } else {
            "Bearish"
        };
        println!(
            " - [{}]: {} ({}) - Conf: {:.2}",
            dt, m.pattern_name, sentiment, m.confidence
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_doji() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![
                Bar {
                    symbol: "AAPL".to_string(),
                    market: "equities".to_string(),
                    timeframe: "1d".to_string(),
                    timestamp_unix_ms: 1000,
                    open: 100.0,
                    high: 110.0,
                    low: 90.0,
                    close: 95.0,
                    volume: 1000.0,
                },
                Bar {
                    symbol: "AAPL".to_string(),
                    market: "equities".to_string(),
                    timeframe: "1d".to_string(),
                    timestamp_unix_ms: 2000,
                    open: 100.0,
                    high: 105.0,
                    low: 95.0,
                    close: 100.001, // Adjusted closer so (0.001 / 10.0) <= 0.001
                    volume: 1000.0,
                },
            ],
        };
        let config = CandlestickPatternsConfig::default();
        let report = analyze_candlestick_patterns(&series, config).unwrap();
        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].pattern_name, "Doji");
    }

    #[test]
    fn test_engulfing() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![
                Bar {
                    symbol: "AAPL".to_string(),
                    market: "equities".to_string(),
                    timeframe: "1d".to_string(),
                    timestamp_unix_ms: 1000,
                    open: 100.0,
                    high: 105.0,
                    low: 95.0,
                    close: 98.0, // Red candle
                    volume: 1000.0,
                },
                Bar {
                    symbol: "AAPL".to_string(),
                    market: "equities".to_string(),
                    timeframe: "1d".to_string(),
                    timestamp_unix_ms: 2000,
                    open: 97.0,
                    high: 110.0,
                    low: 96.0,
                    close: 105.0, // Green candle that engulfs previous body
                    volume: 1000.0,
                },
            ],
        };
        let config = CandlestickPatternsConfig::default();
        let report = analyze_candlestick_patterns(&series, config).unwrap();
        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].pattern_name, "Bullish Engulfing");
        assert!(report.matches[0].bullish);
    }
}
