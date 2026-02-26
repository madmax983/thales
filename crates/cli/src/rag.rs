use contracts::{MarketAnalysis, TradeIntent};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub intent: TradeIntent,
    pub market_analysis: MarketAnalysis,
    pub outcome: Option<f64>, // PnL or score, optional
}

#[derive(Debug, Clone)]
pub struct HistoricalPerformance {
    pub count: usize,
    pub win_rate: f64,
    pub avg_pnl: f64,
}

pub fn analyze_performance(entries: &[HistoryEntry]) -> HistoricalPerformance {
    // Only analyze completed trades (where outcome is known)
    let completed: Vec<&HistoryEntry> = entries
        .iter()
        .filter(|e| e.outcome.is_some())
        .collect();

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

pub fn find_similar_trades(
    current_analysis: &MarketAnalysis,
    history_path: &Path,
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
            similar_trades.push(entry);
        }
    }

    Ok(similar_trades)
}

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
        perf.count, same_symbol_count, perf.win_rate, perf.avg_pnl * 100.0
    )
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

        let similar = find_similar_trades(&current_analysis, history_file.path())?;

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
    fn test_no_history_file() -> Result<()> {
        let current_analysis = create_dummy_analysis("AAPL", "Trending Up", "Low");
        let path = Path::new("non_existent_file.json");
        let similar = find_similar_trades(&current_analysis, path)?;
        assert!(similar.is_empty());
        Ok(())
    }

    #[test]
    fn test_corrupted_history_file() -> Result<()> {
        let mut history_file = NamedTempFile::new()?;
        write!(history_file, "this is not json")?;

        let current_analysis = create_dummy_analysis("AAPL", "Trending Up", "Low");
        let similar = find_similar_trades(&current_analysis, history_file.path());
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
