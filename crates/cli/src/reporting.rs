use crate::rag::HistoryEntry;
use anyhow::{Context, Result};
use chrono::TimeZone;
use contracts::MarketAnalysis;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

pub fn generate_report(
    analysis: &MarketAnalysis,
    similar_trades: &[HistoryEntry],
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
            .map(|l| format!("{}", l))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let research_section = match (&analysis.research_summary, &analysis.news_summary) {
        (Some(r), Some(n)) => format!("**Research**:\n{}\n\n**News**:\n{}", r, n),
        (Some(r), None) => format!("**Research**:\n{}", r),
        (None, Some(n)) => format!("**News**:\n{}", n),
        (None, None) => "No external research available. (Placeholder for search_research)".to_string(),
    };

    let history_section = crate::rag::summarize_history(similar_trades, &analysis.symbol);

    let volatility_display = if analysis.volatility == "Extreme" {
        "**EXTREME (Unusual Activity)**".to_string()
    } else {
        analysis.volatility.clone()
    };

    let recommendation = analysis.recommendation.clone().unwrap_or_else(|| "None".to_string());
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
             if inner.ends_with(')') {
                 return Some(inner[..inner.len() - 1].to_string());
             }
             return Some(inner.to_string());
        }
        // Case 3: "**ALERT: Regime Change Detected!** (Previous: X, Current: Y)"
        if trimmed.starts_with("**ALERT: Regime Change Detected!**") {
            if let Some(pos) = trimmed.rfind("Current: ") {
                let rest = &trimmed[pos + 9..];
                if rest.ends_with(')') {
                    return Some(rest[..rest.len() - 1].to_string());
                }
                return Some(rest.to_string());
            }
        }
    }
    None
}

pub fn append_to_signals_md(path: &Path, content: &str) -> Result<()> {
    append_to_file(path, content)
}

pub fn append_to_file(path: &Path, content: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .context(format!("Failed to open {} for appending", path.display()))?;

    write!(file, "{}", content).context(format!("Failed to write to {}", path.display()))?;
    Ok(())
}

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
