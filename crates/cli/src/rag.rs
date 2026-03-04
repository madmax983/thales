//! Retrieval-Augmented Generation (search history) for historical trade context.
//!
//! This module provides functions to search and analyze a historical database
//! of past trades. By retrieving past trades that occurred under similar market
//! conditions (e.g., same regime, similar volatility), the trading agent can
//! make more informed decisions about whether to execute a new trade.
//!
//! The core functionality revolves around [`find_similar_trades`], which matches
//! a current [`MarketAnalysis`] against a JSON file of [`HistoryEntry`] records,
//! and [`analyze_performance`], which calculates the win rate and average PnL
//! of those similar trades.

use anyhow::Result;
use contracts::{MarketAnalysis, TradeIntent};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// A record of a past trade and the market context at the time it was generated.
///
/// This struct is used to build the historical database for the search history system.
/// It contains the original trade intent, the market analysis that led to it,
/// and the final outcome (PnL) if the trade was completed.
///
/// # Examples
///
/// ```rust
/// use contracts::{MarketAnalysis, TradeIntent};
/// use thales_cli::rag::HistoryEntry;
///
/// let entry = HistoryEntry {
///     intent: TradeIntent::default(),
///     market_analysis: MarketAnalysis {
///         symbol: "BTCUSD".to_string(),
///         market: "crypto".to_string(),
///         regime: "Trending Up".to_string(),
///         sentiment: "Bullish".to_string(),
///         patterns: vec![],
///         key_levels: vec![],
///         volatility: "Low".to_string(),
///         atr: Some(100.0),
///         research_summary: None,
///         news_summary: None,
///         recommendation: None,
///         confidence: 0.8,
///         timestamp_unix_ms: 1622505600000,
///     },
///     outcome: Some(0.05), // 5% profit
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The trade intent that was generated.
    pub intent: TradeIntent,
    /// The market conditions at the time the intent was generated.
    pub market_analysis: MarketAnalysis,
    /// The final outcome of the trade, typically expressed as a fractional PnL (e.g., 0.05 for 5% profit).
    /// If `None`, the trade is still open or the outcome is unknown.
    pub outcome: Option<f64>,
}

/// A summary of the performance of a set of historical trades.
///
/// This is typically used to aggregate the outcomes of trades found via [`find_similar_trades`].
#[derive(Debug, Clone)]
pub struct HistoricalPerformance {
    /// The number of completed trades analyzed.
    pub count: usize,
    /// The percentage of trades that were profitable (0.0 to 100.0).
    pub win_rate: f64,
    /// The average outcome (PnL) of the trades.
    pub avg_pnl: f64,
}

/// Analyzes a slice of historical entries to determine their aggregate performance.
///
/// Only entries with a `Some` outcome are considered. If no entries have an outcome,
/// it returns a zeroed [`HistoricalPerformance`] struct.
///
/// # Examples
///
/// ```rust
/// use contracts::{MarketAnalysis, TradeIntent};
/// use thales_cli::rag::{HistoryEntry, analyze_performance};
///
/// let entries = vec![
///     HistoryEntry {
///         intent: TradeIntent::default(),
///         market_analysis: MarketAnalysis {
///             symbol: "BTCUSD".to_string(),
///             market: "crypto".to_string(),
///             regime: "Trending Up".to_string(),
///             sentiment: "Bullish".to_string(),
///             patterns: vec![],
///             key_levels: vec![],
///             volatility: "Low".to_string(),
///             atr: Some(100.0),
///             research_summary: None,
///             news_summary: None,
///             recommendation: None,
///             confidence: 0.8,
///             timestamp_unix_ms: 1622505600000,
///         },
///         outcome: Some(0.10), // Win
///     },
///     HistoryEntry {
///         intent: TradeIntent::default(),
///         market_analysis: MarketAnalysis {
///             symbol: "BTCUSD".to_string(),
///             market: "crypto".to_string(),
///             regime: "Trending Up".to_string(),
///             sentiment: "Bullish".to_string(),
///             patterns: vec![],
///             key_levels: vec![],
///             volatility: "Low".to_string(),
///             atr: Some(100.0),
///             research_summary: None,
///             news_summary: None,
///             recommendation: None,
///             confidence: 0.8,
///             timestamp_unix_ms: 1622505600000,
///         },
///         outcome: Some(-0.05), // Loss
///     }
/// ];
///
/// let perf = analyze_performance(&entries);
/// assert_eq!(perf.count, 2);
/// assert_eq!(perf.win_rate, 50.0);
/// assert_eq!(perf.avg_pnl, 0.025); // (0.10 - 0.05) / 2
/// ```
pub fn analyze_performance(entries: &[HistoryEntry]) -> HistoricalPerformance {
    // Only analyze completed trades (where outcome is known)
    let completed: Vec<&HistoryEntry> = entries.iter().filter(|e| e.outcome.is_some()).collect();

    let count = completed.len();
    if count == 0 {
        return HistoricalPerformance {
            count: 0,
            win_rate: 0.0,
            avg_pnl: 0.0,
        };
    }

    let wins = completed
        .iter()
        .filter(|t| t.outcome.unwrap_or(0.0) > 0.0)
        .count();
    let win_rate = (wins as f64 / count as f64) * 100.0;
    let avg_outcome = completed
        .iter()
        .map(|t| t.outcome.unwrap_or(0.0))
        .sum::<f64>()
        / count as f64;

    HistoricalPerformance {
        count,
        win_rate,
        avg_pnl: avg_outcome,
    }
}

/// Finds past trades that occurred in market conditions similar to the current analysis.
///
/// It reads a JSON file containing an array of [`HistoryEntry`] records and filters them
/// based on the market, regime, and volatility of the provided `current_analysis`.
/// It can optionally filter by a specific trading strategy.
///
/// # Arguments
///
/// * `current_analysis` - The current market conditions to match against.
/// * `history_path` - The path to the JSON file containing the history. If the file doesn't exist, it returns an empty vector.
/// * `strategy_filter` - An optional strategy name to further filter the results (e.g., "BollingerBands").
///
/// # Examples
///
/// ```rust
/// use std::io::Write;
/// use tempfile::NamedTempFile;
/// use contracts::MarketAnalysis;
/// use thales_cli::rag::{HistoryEntry, find_similar_trades};
///
/// # fn main() -> anyhow::Result<()> {
/// // 1. Create a dummy history file
/// let mut file = NamedTempFile::new()?;
/// let history = r#"[
///     {
///         "intent": { "intent_id": "1", "market": "crypto", "symbol": "BTCUSD", "side": "buy", "size_hint": "1", "confidence": 0.8, "horizon": "1d", "rationale": "", "invalidation": "", "schema_version": "v0", "order_type": "market", "time_in_force": "day", "strategy": "SMA" },
///         "market_analysis": { "symbol": "BTCUSD", "market": "crypto", "regime": "Trending Up", "sentiment": "Bullish", "patterns": [], "key_levels": [], "volatility": "Low", "confidence": 0.9, "timestamp_unix_ms": 1000 },
///         "outcome": 0.05
///     }
/// ]"#;
/// write!(file, "{}", history)?;
///
/// // 2. Create our current analysis to match
/// let current_analysis = MarketAnalysis {
///     symbol: "ETHUSD".to_string(), // Different symbol, but...
///     market: "crypto".to_string(), // Same market
///     regime: "Trending Up".to_string(), // Same regime
///     sentiment: "Bullish".to_string(),
///     patterns: vec![],
///     key_levels: vec![],
///     volatility: "Low".to_string(), // Same volatility
///     atr: None,
///     research_summary: None,
///     news_summary: None,
///     recommendation: None,
///     confidence: 0.8,
///     timestamp_unix_ms: 2000,
/// };
///
/// // 3. Find similar trades
/// let similar = find_similar_trades(&current_analysis, file.path(), Some("SMA"))?;
/// assert_eq!(similar.len(), 1);
/// # Ok(())
/// # }
/// ```
pub fn find_similar_trades(
    current_analysis: &MarketAnalysis,
    history_path: &Path,
    strategy_filter: Option<&str>,
) -> Result<Vec<HistoryEntry>> {
    if !history_path.exists() {
        return Ok(vec![]);
    }

    let raw = fs::read_to_string(history_path)
        .map_err(|e| anyhow::anyhow!("Failed to read history file: {}", e))?;

    let history: Vec<HistoryEntry> = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("Failed to parse history JSON: {}", e))?;

    let mut similar_trades = Vec::new();

    for entry in history {
        // Basic similarity check:
        // 1. Same market
        // 2. Similar regime
        // 3. Similar volatility
        // 4. Optionally same symbol (prioritized in summary but included in list)

        if entry.market_analysis.market == current_analysis.market
            && entry.market_analysis.regime == current_analysis.regime
            && entry.market_analysis.volatility == current_analysis.volatility
        {
            // 5. Strategy Match (if filter provided)
            if let Some(strategy_name) = strategy_filter {
                // Check explicit field first
                if !entry.intent.strategy.is_empty() {
                    if !strategies_match(&entry.intent.strategy, strategy_name) {
                        continue;
                    }
                } else {
                    // Fallback: Check rationale for "Strategy: Name"
                    // Format: "Strategy: {Name} (..."
                    // We check if rationale starts with either the strategy name or its aliases
                    let aliases = get_strategy_aliases(strategy_name);
                    let mut match_found = false;
                    for name in aliases {
                        if entry
                            .intent
                            .rationale
                            .starts_with(&format!("Strategy: {}", name))
                        {
                            match_found = true;
                            break;
                        }
                    }
                    if !match_found {
                        continue;
                    }
                }
            }

            similar_trades.push(entry);
        }
    }

    Ok(similar_trades)
}

/// Counts the number of signals generated for a specific symbol on a given day.
///
/// This is used to enforce daily signal limits (e.g., maximum 3 trades per symbol per day)
/// to prevent overtrading during choppy conditions.
///
/// # Arguments
///
/// * `symbol` - The symbol to count signals for.
/// * `history_path` - Path to the historical trade database.
/// * `reference_ts` - The current timestamp in milliseconds to determine the "day".
///
/// # Examples
///
/// ```rust
/// use std::io::Write;
/// use tempfile::NamedTempFile;
/// use thales_cli::rag::count_todays_signals;
///
/// # fn main() -> anyhow::Result<()> {
/// let mut file = NamedTempFile::new()?;
/// // Two signals on day 0 (timestamp < 86400000)
/// let history = r#"[
///     { "intent": { "intent_id": "1", "market": "crypto", "symbol": "BTCUSD", "side": "buy", "size_hint": "1", "confidence": 0.8, "horizon": "1d", "rationale": "", "invalidation": "", "schema_version": "v0", "order_type": "market", "time_in_force": "day", "strategy": "" }, "market_analysis": { "symbol": "BTCUSD", "market": "crypto", "regime": "", "sentiment": "", "patterns": [], "key_levels": [], "volatility": "", "confidence": 0.0, "timestamp_unix_ms": 1000 }, "outcome": null },
///     { "intent": { "intent_id": "2", "market": "crypto", "symbol": "BTCUSD", "side": "buy", "size_hint": "1", "confidence": 0.8, "horizon": "1d", "rationale": "", "invalidation": "", "schema_version": "v0", "order_type": "market", "time_in_force": "day", "strategy": "" }, "market_analysis": { "symbol": "BTCUSD", "market": "crypto", "regime": "", "sentiment": "", "patterns": [], "key_levels": [], "volatility": "", "confidence": 0.0, "timestamp_unix_ms": 2000 }, "outcome": null }
/// ]"#;
/// write!(file, "{}", history)?;
///
/// let count = count_todays_signals("BTCUSD", file.path(), 3000)?;
/// assert_eq!(count, 2);
/// # Ok(())
/// # }
/// ```
pub fn count_todays_signals(symbol: &str, history_path: &Path, reference_ts: i64) -> Result<usize> {
    if !history_path.exists() {
        return Ok(0);
    }

    let raw = fs::read_to_string(history_path)
        .map_err(|e| anyhow::anyhow!("Failed to read history file: {}", e))?;

    let history: Vec<HistoryEntry> = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("Failed to parse history JSON: {}", e))?;

    let ms_per_day = 24 * 60 * 60 * 1000;
    let today_day_num = reference_ts / ms_per_day;

    let count = history
        .iter()
        .filter(|entry| {
            entry.market_analysis.symbol == symbol
                && (entry.market_analysis.timestamp_unix_ms / ms_per_day) == today_day_num
        })
        .count();

    Ok(count)
}

/// Generates a human-readable summary of historical performance.
///
/// This string is typically appended to the rationale of a new trade intent to provide
/// context on how similar trades have performed in the past.
///
/// # Examples
///
/// ```rust
/// use contracts::{MarketAnalysis, TradeIntent};
/// use thales_cli::rag::{HistoryEntry, summarize_history};
///
/// let entries = vec![
///     HistoryEntry {
///         intent: TradeIntent::default(),
///         market_analysis: MarketAnalysis {
///             symbol: "BTCUSD".to_string(),
///             market: "crypto".to_string(),
///             regime: "Trending Up".to_string(),
///             sentiment: "Bullish".to_string(),
///             patterns: vec![],
///             key_levels: vec![],
///             volatility: "Low".to_string(),
///             atr: Some(100.0),
///             research_summary: None,
///             news_summary: None,
///             recommendation: None,
///             confidence: 0.8,
///             timestamp_unix_ms: 1622505600000,
///         },
///         outcome: Some(0.10),
///     }
/// ];
///
/// let summary = summarize_history(&entries, "BTCUSD");
/// assert_eq!(summary, "Found 1 similar past trades (1 on same symbol). Win Rate: 100.0%. Avg Return: 10.00%");
/// ```
pub fn summarize_history(entries: &[HistoryEntry], current_symbol: &str) -> String {
    let perf = analyze_performance(entries);

    if perf.count == 0 {
        return "No similar past trades found.".to_string();
    }

    let same_symbol_count = entries
        .iter()
        .filter(|e| e.market_analysis.symbol == current_symbol)
        .count();

    format!(
        "Found {} similar past trades ({} on same symbol). Win Rate: {:.1}%. Avg Return: {:.2}%",
        perf.count,
        same_symbol_count,
        perf.win_rate,
        perf.avg_pnl * 100.0
    )
}

fn get_strategy_aliases(name: &str) -> Vec<&str> {
    match name {
        "BollingerBands" => vec!["BollingerBands", "BollingerBandsMeanReversion"],
        "BollingerBandsMeanReversion" => vec!["BollingerBands", "BollingerBandsMeanReversion"],
        _ => vec![name],
    }
}

fn strategies_match(name1: &str, name2: &str) -> bool {
    if name1 == name2 {
        return true;
    }
    let aliases1 = get_strategy_aliases(name1);
    let aliases2 = get_strategy_aliases(name2);

    for a1 in &aliases1 {
        for a2 in &aliases2 {
            if a1 == a2 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_dummy_analysis(symbol: &str, regime: &str, volatility: &str) -> MarketAnalysis {
        MarketAnalysis {
            symbol: symbol.to_string(),
            market: "equities".to_string(),
            regime: regime.to_string(),
            sentiment: "Neutral".to_string(),
            patterns: vec![],
            key_levels: vec![],
            volatility: volatility.to_string(),
            atr: None,
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.5,
            timestamp_unix_ms: 1000,
        }
    }

    fn create_dummy_intent(symbol: &str) -> TradeIntent {
        TradeIntent {
            symbol: symbol.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_find_similar_trades() -> Result<()> {
        let mut history_file = NamedTempFile::new()?;
        let history_data = vec![
            HistoryEntry {
                intent: create_dummy_intent("AAPL"),
                market_analysis: create_dummy_analysis("AAPL", "Trending Up", "Low"),
                outcome: Some(1.0),
            },
            HistoryEntry {
                intent: create_dummy_intent("GOOG"),
                market_analysis: create_dummy_analysis("GOOG", "Trending Up", "Low"),
                outcome: Some(1.0),
            },
            HistoryEntry {
                intent: create_dummy_intent("AAPL"),
                market_analysis: create_dummy_analysis("AAPL", "Ranging", "Low"),
                outcome: Some(-1.0),
            },
        ];

        write!(history_file, "{}", serde_json::to_string(&history_data)?)?;

        let current_analysis = create_dummy_analysis("AAPL", "Trending Up", "Low");

        let similar = find_similar_trades(&current_analysis, history_file.path(), None)?;

        // Should find AAPL (exact match) and GOOG (context match).
        // Should NOT find second AAPL (different regime).
        assert_eq!(similar.len(), 2);

        // Sort or check existence
        let has_aapl = similar.iter().any(|e| e.market_analysis.symbol == "AAPL");
        let has_goog = similar.iter().any(|e| e.market_analysis.symbol == "GOOG");
        assert!(has_aapl);
        assert!(has_goog);

        Ok(())
    }

    #[test]
    fn test_find_similar_trades_alias_matching() -> Result<()> {
        let mut history_file = NamedTempFile::new()?;
        let mut entry = HistoryEntry {
            intent: create_dummy_intent("AAPL"),
            market_analysis: create_dummy_analysis("AAPL", "Trending Up", "Low"),
            outcome: Some(1.0),
        };
        // Old name in history
        entry.intent.strategy = "BollingerBandsMeanReversion".to_string();

        let history_data = vec![entry];
        write!(history_file, "{}", serde_json::to_string(&history_data)?)?;

        let current_analysis = create_dummy_analysis("AAPL", "Trending Up", "Low");

        // New name in search
        let similar = find_similar_trades(
            &current_analysis,
            history_file.path(),
            Some("BollingerBands"),
        )?;

        assert_eq!(similar.len(), 1, "Should find trade despite name change");
        assert_eq!(similar[0].intent.strategy, "BollingerBandsMeanReversion");

        Ok(())
    }

    #[test]
    fn test_no_history_file() -> Result<()> {
        let current_analysis = create_dummy_analysis("AAPL", "Trending Up", "Low");
        let path = Path::new("non_existent_file.json");
        let similar = find_similar_trades(&current_analysis, path, None)?;
        assert!(similar.is_empty());
        Ok(())
    }

    #[test]
    fn test_corrupted_history_file() -> Result<()> {
        let mut history_file = NamedTempFile::new()?;
        write!(history_file, "this is not json")?;

        let current_analysis = create_dummy_analysis("AAPL", "Trending Up", "Low");
        let similar = find_similar_trades(&current_analysis, history_file.path(), None);
        assert!(similar.is_err());

        let count = count_todays_signals("AAPL", history_file.path(), 1000);
        assert!(count.is_err());

        Ok(())
    }

    #[test]
    fn test_summarize_history() {
        let entries = vec![
            HistoryEntry {
                intent: create_dummy_intent("AAPL"),
                market_analysis: create_dummy_analysis("AAPL", "Trending Up", "Low"),
                outcome: Some(0.10), // Win 10%
            },
            HistoryEntry {
                intent: create_dummy_intent("GOOG"),
                market_analysis: create_dummy_analysis("GOOG", "Trending Up", "Low"),
                outcome: Some(-0.05), // Loss 5%
            },
        ];

        let summary = summarize_history(&entries, "AAPL");
        assert!(summary.contains("Found 2 similar past trades"));
        assert!(summary.contains("(1 on same symbol)"));
        assert!(summary.contains("Win Rate: 50.0%"));
        assert!(summary.contains("Avg Return: 2.50%")); // (0.10 - 0.05) / 2 = 0.025 = 2.5%
    }
}
