//! Report Generation and File Management
//!
//! This module handles the formatting and persistence of market analysis and trading data.
//! It is responsible for transforming raw structs like [`MarketAnalysis`] into
//! human-readable Markdown reports and appending them to the appropriate tracking files
//! (e.g., `Signals.md`, `Market_Regime.md`).
//!
//! # Core Responsibilities
//!
//! - **Formatting**: Generating structured Markdown blocks for various analytical perspectives
//!   (general analysis, regime, volatility, research).
//! - **Persistence**: Safely appending reports to log files.
//! - **State Tracking**: Reading previous entries (like the last known market regime)
//!   to detect changes and generate alerts.
//!
//! # Note
//!
//! This module intentionally interacts with the filesystem to maintain a persistent
//! "paper trail" for the trading agent's decisions.

use crate::search_history::HistoryEntry;
use anyhow::{Context, Result};
use chrono::TimeZone;
use contracts::MarketAnalysis;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Generates a comprehensive, human-readable Markdown report of the market analysis.
///
/// This report includes the market regime (with alerts if it changed from the previous
/// state), volatility, strategy recommendations, identified patterns, key support/resistance
/// levels, and any external research/news. It also summarizes historical performance
/// context.
///
/// # Arguments
///
/// * `analysis` - The [`MarketAnalysis`] containing the technical evaluation.
/// * `similar_trades` - A slice of [`HistoryEntry`] providing historical context.
/// * `previous_regime` - An optional string indicating the last known market regime, used
///   to detect and alert on regime changes.
///
/// # Returns
///
/// Returns a formatted Markdown string containing the full report.
///
/// # Examples
///
/// ```rust
/// use contracts::MarketAnalysis;
/// use thales_cli::reporting::generate_report;
///
/// let analysis = MarketAnalysis {
///     symbol: String::from("BTCUSD"),
///     market: String::from("crypto"),
///     regime: String::from("Trending Up"),
///     sentiment: String::from("Bullish"),
///     patterns: vec![String::from("Bullish Engulfing")],
///     key_levels: vec![30000.0, 31000.0],
///     volatility: String::from("High"),
///     atr: Some(500.0),
///     research_summary: None,
///     news_summary: None,
///     recommendation: Some(String::from("Trend Following (Long)")),
///     confidence: 0.85,
///     timestamp_unix_ms: 1622505600000,
/// };
///
/// let report = generate_report(&analysis, &[], Some("Ranging"));
/// assert!(report.contains("**ALERT: Regime Change Detected!**"));
/// assert!(report.contains("Trend Following (Long)"));
/// ```
pub fn generate_report(
    analysis: &MarketAnalysis,
    similar_trades: &[HistoryEntry],
    previous_regime: Option<&str>,
) -> String {
    let regime_change = if let Some(prev) = previous_regime {
        if prev != analysis.regime {
            format!(
                "**ALERT: Regime Change Detected!** (Previous: {prev}, Current: {})",
                analysis.regime
            )
        } else {
            format!("Regime Unchanged ({})", analysis.regime)
        }
    } else {
        format!("Regime: {}", analysis.regime)
    };

    let patterns_str = if analysis.patterns.is_empty() {
        "None detected".to_string()
    } else {
        analysis.patterns.join(", ")
    };

    let levels_str = if analysis.key_levels.is_empty() {
        "None identified".to_string()
    } else {
        analysis
            .key_levels
            .iter()
            .map(|l| format!("{}", l))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let research_section = match (&analysis.research_summary, &analysis.news_summary) {
        (Some(r), Some(n)) => format!("**Research**:\n{}\n\n**News**:\n{}", r, n),
        (Some(r), None) => format!("**Research**:\n{}", r),
        (None, Some(n)) => format!("**News**:\n{}", n),
        (None, None) => {
            "No external research available. (Placeholder for search_research)".to_string()
        }
    };

    let history_section =
        crate::search_history::summarize_history(similar_trades, &analysis.symbol);

    let volatility_display = if analysis.volatility == "Extreme" {
        "**EXTREME (Unusual Activity)**".to_string()
    } else {
        analysis.volatility.clone()
    };

    let recommendation = analysis
        .recommendation
        .clone()
        .unwrap_or_else(|| "None".to_string());
    let json_block = serde_json::to_string_pretty(analysis).unwrap_or_default();

    format!(
        r#"
## Market Analysis Report - {} - {}

**Timestamp (ms)**: {}
**Confidence**: {:.2}%

### 1. Market Regime
{}
*Sentiment*: {}

### 2. Volatility
*Assessment*: {}

### 3. Strategy Recommendation
**{}**

### 4. Patterns & Price Action
*Patterns*: {}

### 5. Key Levels
*Support/Resistance*: {}

### 6. Research & Context
{}
*Historical Context*: {}

```json
{}
```

---
"#,
        analysis.market,
        analysis.symbol,
        analysis.timestamp_unix_ms,
        analysis.confidence * 100.0,
        regime_change,
        analysis.sentiment,
        volatility_display,
        recommendation,
        patterns_str,
        levels_str,
        research_section,
        history_section,
        json_block
    )
}

/// Reads the last known market regime for a specific symbol from a markdown file.
///
/// This function parses the provided file (typically `Signals.md` or similar) backwards
/// to find the most recent "Market Analysis Report" block for the given symbol and
/// extracts the recorded regime.
///
/// # Arguments
///
/// * `path` - The path to the markdown file to read.
/// * `symbol` - The symbol to search for.
///
/// # Returns
///
/// Returns `Some(String)` containing the regime if found, or `None` if the file doesn't
/// exist, the symbol isn't found, or the regime cannot be extracted.
///
/// # Examples
///
/// ```rust
/// use std::fs::File;
/// use std::io::Write;
/// use tempfile::NamedTempFile;
/// use thales_cli::reporting::read_last_regime;
///
/// let mut file = NamedTempFile::new().unwrap();
/// writeln!(file, "## Market Analysis Report - crypto - BTCUSD\nRegime: Trending Up").unwrap();
///
/// let regime = read_last_regime(file.path(), "BTCUSD");
/// assert_eq!(regime.unwrap(), "Trending Up");
/// ```
pub fn read_last_regime(path: &Path, symbol: &str) -> Option<String> {
    if !path.exists() {
        return None;
    }

    let content = fs::read_to_string(path).ok()?;

    // Split by "## Market Analysis Report" to get blocks
    let blocks: Vec<&str> = content.split("## Market Analysis Report").collect();

    // Iterate backwards to find the last valid block
    for block in blocks.iter().rev() {
        if block.trim().is_empty() {
            continue;
        }
        // Check if block contains symbol
        if !block.contains(symbol) {
            continue;
        }
        if let Some(regime) = extract_regime_from_block(block) {
            return Some(regime);
        }
    }
    None
}

fn extract_regime_from_block(block: &str) -> Option<String> {
    for line in block.lines() {
        let trimmed = line.trim();
        // Case 1: "Regime: Trending Up"
        if trimmed.starts_with("Regime:") {
            return Some(trimmed.trim_start_matches("Regime:").trim().to_string());
        }
        // Case 2: "Regime Unchanged (Trending Up)"
        if trimmed.starts_with("Regime Unchanged (") {
            let inner = trimmed.trim_start_matches("Regime Unchanged (");
            if let Some(stripped) = inner.strip_suffix(')') {
                return Some(stripped.to_string());
            }
            return Some(inner.to_string());
        }
        // Case 3: "**ALERT: Regime Change Detected!** (Previous: X, Current: Y)"
        if trimmed.starts_with("**ALERT: Regime Change Detected!**")
            && let Some(pos) = trimmed.rfind("Current: ")
        {
            let rest = &trimmed[pos + 9..];
            if let Some(stripped) = rest.strip_suffix(')') {
                return Some(stripped.to_string());
            }
            return Some(rest.to_string());
        }
    }
    None
}

/// Appends text content to a specific markdown file, typically used for signals.
///
/// This is a convenience wrapper around [`append_to_file`].
///
/// # Arguments
///
/// * `path` - The path to the markdown file.
/// * `content` - The string content to append.
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if the file cannot be opened or written to.
pub fn append_to_signals_md(path: &Path, content: &str) -> Result<()> {
    append_to_file(path, content)
}

/// Safely appends text content to a file, creating it if it does not exist.
///
/// # Arguments
///
/// * `path` - The path to the file.
/// * `content` - The string content to append.
///
/// # Returns
///
/// Returns `Ok(())` on success, or an `anyhow::Error` with context if the operation fails.
///
/// # Examples
///
/// ```rust
/// use std::fs;
/// use tempfile::NamedTempFile;
/// use thales_cli::reporting::append_to_file;
///
/// let file = NamedTempFile::new().unwrap();
/// let path = file.path().to_path_buf();
///
/// append_to_file(&path, "Hello, Market!").unwrap();
///
/// let content = fs::read_to_string(&path).unwrap();
/// assert!(content.contains("Hello, Market!"));
/// ```
pub fn append_to_file(path: &Path, content: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .context(format!("Failed to open {} for appending", path.display()))?;

    write!(file, "{}", content).context(format!("Failed to write to {}", path.display()))?;
    Ok(())
}

/// Generates a concise markdown snippet summarizing the current market regime.
///
/// # Examples
///
/// ```rust
/// use contracts::MarketAnalysis;
/// use thales_cli::reporting::generate_regime_report;
///
/// let analysis = MarketAnalysis {
///     symbol: String::from("ETHUSD"),
///     market: String::from("crypto"),
///     regime: String::from("Ranging"),
///     sentiment: String::from("Neutral"),
///     patterns: vec![],
///     key_levels: vec![],
///     volatility: String::from("Low"),
///     atr: None,
///     research_summary: None,
///     news_summary: None,
///     recommendation: None,
///     confidence: 0.5,
///     timestamp_unix_ms: 1622505600000,
/// };
///
/// let report = generate_regime_report(&analysis);
/// assert!(report.contains("**Regime**: Ranging"));
/// ```
pub fn generate_regime_report(analysis: &MarketAnalysis) -> String {
    let dt = chrono::Utc
        .timestamp_millis_opt(analysis.timestamp_unix_ms)
        .unwrap();
    let formatted_date = dt.format("%Y-%m-%d %H:%M:%S").to_string();

    format!(
        "\n### {} - {} ({})\n**Regime**: {}\n**Sentiment**: {}\n**Confidence**: {:.2}%\n",
        analysis.symbol,
        formatted_date,
        analysis.market,
        analysis.regime,
        analysis.sentiment,
        analysis.confidence * 100.0
    )
}

/// Generates a concise markdown snippet summarizing the current volatility state.
///
/// # Examples
///
/// ```rust
/// use contracts::MarketAnalysis;
/// use thales_cli::reporting::generate_volatility_report;
///
/// let analysis = MarketAnalysis {
///     symbol: String::from("AAPL"),
///     market: String::from("equities"),
///     regime: String::from("Trending Up"),
///     sentiment: String::from("Bullish"),
///     patterns: vec![],
///     key_levels: vec![],
///     volatility: String::from("High"),
///     atr: Some(2.5),
///     research_summary: None,
///     news_summary: None,
///     recommendation: Some(String::from("Wait")),
///     confidence: 0.8,
///     timestamp_unix_ms: 1622505600000,
/// };
///
/// let report = generate_volatility_report(&analysis);
/// assert!(report.contains("**Volatility**: High"));
/// assert!(report.contains("**ATR**: 2.50"));
/// ```
pub fn generate_volatility_report(analysis: &MarketAnalysis) -> String {
    let dt = chrono::Utc
        .timestamp_millis_opt(analysis.timestamp_unix_ms)
        .unwrap();
    let formatted_date = dt.format("%Y-%m-%d %H:%M:%S").to_string();

    let atr_display = analysis
        .atr
        .map(|a| format!("{:.2}", a))
        .unwrap_or_else(|| "N/A".to_string());
    format!(
        "\n### {} - {} ({})\n**Volatility**: {}\n**ATR**: {}\n**Assessment**: {}\n",
        analysis.symbol,
        formatted_date,
        analysis.market,
        analysis.volatility,
        atr_display,
        analysis.recommendation.clone().unwrap_or_default()
    )
}

/// Generates a concise markdown snippet summarizing external research and news.
///
/// # Examples
///
/// ```rust
/// use contracts::MarketAnalysis;
/// use thales_cli::reporting::generate_research_report;
///
/// let analysis = MarketAnalysis {
///     symbol: String::from("TSLA"),
///     market: String::from("equities"),
///     regime: String::from("Trending Down"),
///     sentiment: String::from("Bearish"),
///     patterns: vec![],
///     key_levels: vec![],
///     volatility: String::from("Extreme"),
///     atr: None,
///     research_summary: Some(String::from("Earnings miss expected.")),
///     news_summary: Some(String::from("CEO sells shares.")),
///     recommendation: None,
///     confidence: 0.9,
///     timestamp_unix_ms: 1622505600000,
/// };
///
/// let report = generate_research_report(&analysis);
/// assert!(report.contains("**Research**: Earnings miss expected."));
/// assert!(report.contains("**News**: CEO sells shares."));
/// ```
pub fn generate_research_report(analysis: &MarketAnalysis) -> String {
    let dt = chrono::Utc
        .timestamp_millis_opt(analysis.timestamp_unix_ms)
        .unwrap();
    let formatted_date = dt.format("%Y-%m-%d %H:%M:%S").to_string();

    let research = analysis
        .research_summary
        .clone()
        .unwrap_or_else(|| "None".to_string());
    let news = analysis
        .news_summary
        .clone()
        .unwrap_or_else(|| "None".to_string());
    format!(
        "\n### {} - {} ({})\n**Research**: {}\n**News**: {}\n",
        analysis.symbol, formatted_date, analysis.market, research, news
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_regime() {
        let block1 = "Regime: Trending Up";
        assert_eq!(
            extract_regime_from_block(block1),
            Some("Trending Up".to_string())
        );

        let block2 =
            "**ALERT: Regime Change Detected!** (Previous: Ranging, Current: Trending Down)";
        assert_eq!(
            extract_regime_from_block(block2),
            Some("Trending Down".to_string())
        );

        let block3 = "Regime Unchanged (Trending Up)";
        assert_eq!(
            extract_regime_from_block(block3),
            Some("Trending Up".to_string())
        );

        let block_multiline = "\nHeader\nRegime: Trending Up\nFooter";
        assert_eq!(
            extract_regime_from_block(block_multiline),
            Some("Trending Up".to_string())
        );
    }

    #[test]
    fn test_generate_report() {
        let analysis = MarketAnalysis {
            symbol: "AAPL".to_string(),
            market: "equities".to_string(),
            regime: "Trending Up".to_string(),
            sentiment: "Bullish".to_string(),
            patterns: vec!["Doji".to_string()],
            key_levels: vec![100.0, 110.0],
            volatility: "Low".to_string(),
            atr: None,
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.9,
            timestamp_unix_ms: 1600000000000,
        };

        let report = generate_report(&analysis, &[], Some("Ranging"));
        assert!(report.contains("**ALERT: Regime Change Detected!**"));
        assert!(report.contains("Doji"));
        assert!(report.contains("100, 110"));
        assert!(report.contains("```json"));
        assert!(report.contains("\"symbol\": \"AAPL\""));
    }
}
