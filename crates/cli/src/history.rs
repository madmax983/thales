use crate::rag::HistoryEntry;
use anyhow::{Context, Result};
use contracts::Bar;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn parse_horizon(h: &str) -> i64 {
    let h = h.trim();
    if h.ends_with('h') {
        if let Ok(v) = h.trim_end_matches('h').parse::<i64>() {
            return v * 3600_000;
        }
    } else if h.ends_with('d') {
        if let Ok(v) = h.trim_end_matches('d').parse::<i64>() {
            return v * 86_400_000;
        }
    } else if h.ends_with('m')
        && let Ok(v) = h.trim_end_matches('m').parse::<i64>() {
            return v * 60_000;
        }
    // Default 24h
    86_400_000
}

pub fn update_history<F>(history_path: &Path, fetch_bars: F) -> Result<usize>
where
    F: Fn(&str, &str) -> Result<Vec<Bar>>,
{
    if !history_path.exists() {
        return Ok(0);
    }

    let raw = fs::read_to_string(history_path).context("Failed to read history file")?;
    let mut history: Vec<HistoryEntry> =
        serde_json::from_str(&raw).context("Failed to parse history JSON")?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System time error")?
        .as_millis() as i64;

    let mut updated_count = 0;

    for entry in &mut history {
        if entry.outcome.is_none() {
            let horizon_ms = parse_horizon(&entry.intent.horizon);
            // Use analysis timestamp as signal time
            let entry_ts = entry.market_analysis.timestamp_unix_ms;
            let exit_ts = entry_ts + horizon_ms;

            if now > exit_ts {
                // Fetch bars covering the trade period
                // We fetch 1h bars by default as it covers most horizons reasonably well
                let bars = match fetch_bars(&entry.intent.symbol, "1h") {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("Failed to fetch bars for {}: {}", entry.intent.symbol, e);
                        continue;
                    }
                };

                if bars.is_empty() {
                    continue;
                }

                // Find entry price (closest bar to entry_ts)
                let entry_bar = bars
                    .iter()
                    .min_by_key(|b| (b.timestamp_unix_ms - entry_ts).abs());
                // Find exit price (closest bar to exit_ts)
                let exit_bar = bars
                    .iter()
                    .min_by_key(|b| (b.timestamp_unix_ms - exit_ts).abs());

                if let (Some(start), Some(end)) = (entry_bar, exit_bar) {
                    // Check if bars are reasonably close to desired timestamps (e.g. within 2 hours)
                    let tolerance = 7200_000; // 2 hours
                    if (start.timestamp_unix_ms - entry_ts).abs() < tolerance
                        && (end.timestamp_unix_ms - exit_ts).abs() < tolerance
                    {
                        let entry_price = start.open; // Assume open of the bar closest to signal time
                        let exit_price = end.close; // Assume close of the bar closest to exit time

                        let direction = if entry.intent.side == "buy" {
                            1.0
                        } else {
                            -1.0
                        };
                        let ret = (exit_price - entry_price) / entry_price * direction;

                        entry.outcome = Some(ret);
                        updated_count += 1;
                        println!(
                            "Updated history for {} ({}): Return {:.2}%",
                            entry.intent.symbol,
                            entry.intent.side,
                            ret * 100.0
                        );
                    }
                }
            }
        }
    }

    if updated_count > 0 {
        let new_json = serde_json::to_string_pretty(&history)?;
        fs::write(history_path, new_json)?;
    }

    Ok(updated_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{MarketAnalysis, TradeIntent};
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_update_history_calculation() -> Result<()> {
        let mut history_file = NamedTempFile::new()?;
        let now_ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
        let entry_ts = now_ms - 90_000_000; // 25 hours ago

        let entry = HistoryEntry {
            intent: TradeIntent {
                symbol: "BTCUSD".to_string(),
                side: "buy".to_string(),
                horizon: "1d".to_string(),
                intent_id: format!("test:{}", entry_ts),
                ..Default::default()
            },
            market_analysis: MarketAnalysis {
                symbol: "BTCUSD".to_string(),
                market: "crypto".to_string(),
                regime: "Trending".to_string(),
                sentiment: "Bullish".to_string(),
                patterns: vec![],
                key_levels: vec![],
                volatility: "Low".to_string(),
                atr: None,
                research_summary: None,
                news_summary: None,
                recommendation: None,
                confidence: 0.8,
                timestamp_unix_ms: entry_ts,
            },
            outcome: None,
        };

        write!(history_file, "[{}]", serde_json::to_string(&entry)?)?;

        let fetch_bars = |_sym: &str, _tf: &str| -> Result<Vec<Bar>> {
            // Return bars covering start and end
            // Entry: 100.0, Exit: 110.0 -> +10%
            Ok(vec![
                Bar {
                    symbol: "BTCUSD".to_string(),
                    market: "crypto".to_string(),
                    timeframe: "1h".to_string(),
                    timestamp_unix_ms: entry_ts,
                    open: 100.0,
                    high: 101.0,
                    low: 99.0,
                    close: 100.0,
                    volume: 1.0,
                },
                Bar {
                    symbol: "BTCUSD".to_string(),
                    market: "crypto".to_string(),
                    timeframe: "1h".to_string(),
                    timestamp_unix_ms: entry_ts + 86_400_000,
                    open: 109.0,
                    high: 111.0,
                    low: 109.0,
                    close: 110.0,
                    volume: 1.0,
                },
            ])
        };

        let updated = update_history(history_file.path(), fetch_bars)?;
        assert_eq!(updated, 1);

        let content = fs::read_to_string(history_file.path())?;
        let history: Vec<HistoryEntry> = serde_json::from_str(&content)?;
        let outcome = history[0].outcome.unwrap();

        // (110 - 100) / 100 = 0.1
        assert!((outcome - 0.1).abs() < 1e-6);

        Ok(())
    }
}
