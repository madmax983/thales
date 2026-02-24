use anyhow::{Context, Result};
use contracts::{MarketAnalysis, TradeIntent};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

pub fn generate_report(
    analysis: &MarketAnalysis,
    similar_trades: &[TradeIntent],
    previous_regime: Option<&str>,
) -> String {
    let regime_change = if let Some(prev) = previous_regime {
        if prev != analysis.regime {
            format!("**ALERT: Regime Change Detected!** (Previous: {prev}, Current: {})", analysis.regime)
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
            .map(|l| format!("{:.2}", l))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let research_section = match (&analysis.research_summary, &analysis.news_summary) {
        (Some(r), Some(n)) => format!("**Research**:\n{}\n\n**News**:\n{}", r, n),
        (Some(r), None) => format!("**Research**:\n{}", r),
        (None, Some(n)) => format!("**News**:\n{}", n),
        (None, None) => "No external research available. (Placeholder for search_research)".to_string(),
    };

    let history_section = if similar_trades.is_empty() {
        "No similar historical trades found.".to_string()
    } else {
        format!("Found {} similar past trades.", similar_trades.len())
    };

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

### 3. Patterns & Price Action
*Patterns*: {}

### 4. Key Levels
*Support/Resistance*: {}

### 5. Research & Context
{}
*Historical Context*: {}

---
"#,
        analysis.market,
        analysis.symbol,
        analysis.timestamp_unix_ms,
        analysis.confidence * 100.0,
        regime_change,
        analysis.sentiment,
        analysis.volatility,
        patterns_str,
        levels_str,
        research_section,
        history_section
    )
}

pub fn read_last_regime(path: &Path) -> Option<String> {
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
             let inner = trimmed.trim_start_matches("Regime Unchanged (").trim_end_matches(')');
             return Some(inner.to_string());
        }
        // Case 3: "**ALERT: Regime Change Detected!** (Previous: X, Current: Y)"
        if trimmed.starts_with("**ALERT: Regime Change Detected!**") {
            if let Some(pos) = trimmed.rfind("Current: ") {
                let rest = &trimmed[pos + 9..];
                return Some(rest.trim_end_matches(')').to_string());
            }
        }
    }
    None
}

pub fn append_to_signals_md(path: &Path, content: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .context("Failed to open Signals.md for appending")?;

    write!(file, "{}", content).context("Failed to write to Signals.md")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_regime() {
        let block1 = "Regime: Trending Up";
        assert_eq!(extract_regime_from_block(block1), Some("Trending Up".to_string()));

        let block2 = "**ALERT: Regime Change Detected!** (Previous: Ranging, Current: Trending Down)";
        assert_eq!(extract_regime_from_block(block2), Some("Trending Down".to_string()));

        let block3 = "Regime Unchanged (Trending Up)";
        assert_eq!(extract_regime_from_block(block3), Some("Trending Up".to_string()));

        let block_multiline = "\nHeader\nRegime: Trending Up\nFooter";
        assert_eq!(extract_regime_from_block(block_multiline), Some("Trending Up".to_string()));
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
            confidence: 0.9,
            timestamp_unix_ms: 1600000000000,
        };

        let report = generate_report(&analysis, &[], Some("Ranging"));
        assert!(report.contains("**ALERT: Regime Change Detected!**"));
        assert!(report.contains("Doji"));
        assert!(report.contains("100.00, 110.00"));
    }
}
