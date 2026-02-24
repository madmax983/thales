use contracts::{Bar, BarSeries, TradeIntent, MarketAnalysis};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;
use thales_cli::signals;
use thales_cli::rag::HistoryEntry;
use serde_json::json;

fn create_bar(symbol: &str, timestamp: i64, close: f64) -> Bar {
    Bar {
        symbol: symbol.to_string(),
        market: "equities".to_string(),
        timeframe: "1m".to_string(),
        timestamp_unix_ms: timestamp,
        open: close,
        high: close * 1.01,
        low: close * 0.99,
        close,
        volume: 1000.0,
    }
}

fn create_history_entry(symbol: &str, timestamp: i64, regime: &str, volatility: &str) -> HistoryEntry {
    HistoryEntry {
        intent: TradeIntent {
            symbol: symbol.to_string(),
            ..Default::default()
        },
        market_analysis: MarketAnalysis {
            symbol: symbol.to_string(),
            market: "equities".to_string(),
            regime: regime.to_string(),
            sentiment: "Neutral".to_string(),
            patterns: vec![],
            key_levels: vec![],
            volatility: volatility.to_string(),
            atr: None,
            confidence: 0.0,
            timestamp_unix_ms: timestamp,
            research_summary: None,
            news_summary: None,
        },
        outcome: None,
    }
}

#[tokio::test]
async fn test_signal_generation_limit() {
    // 1. Setup History with 3 signals for today
    let dir = tempdir().unwrap();
    let history_path = dir.path().join("history.json");

    let symbol = "AAPL";
    let now = 1600000000000; // Fixed timestamp (approx 2020)
    // Same day signals
    let history = vec![
        create_history_entry(symbol, now - 1000, "Unknown", "Unknown"),
        create_history_entry(symbol, now - 2000, "Unknown", "Unknown"),
        create_history_entry(symbol, now - 3000, "Unknown", "Unknown"),
    ];

    fs::write(&history_path, serde_json::to_string(&history).unwrap()).unwrap();

    // 2. Create BarSeries that triggers a signal
    let mut bars = Vec::new();
    // 20 bars stable
    for i in 0..20 {
        bars.push(create_bar(symbol, now + i * 60000, 100.0));
    }
    // Spike to trigger Entry
    bars.push(create_bar(symbol, now + 20 * 60000, 110.0));

    let series = BarSeries {
        schema_version: "v0".to_string(),
        bars,
    };

    // 3. Generate Signals
    let intents = signals::generate_signals(&series, "BollingerBands", Some(&history_path)).await.unwrap();

    // 4. Assert Limit Reached (should be empty)
    assert!(intents.is_empty(), "Should not generate signal if 3 already exist for today");
}

#[tokio::test]
async fn test_signal_generation_success() {
    // 1. Setup Empty History
    let dir = tempdir().unwrap();
    let history_path = dir.path().join("history.json");
    fs::write(&history_path, "[]").unwrap();

    let symbol = "AAPL";
    let now = 1600000000000;

    // 2. Create BarSeries that triggers a signal
    let mut bars = Vec::new();
    for i in 0..20 {
        bars.push(create_bar(symbol, now + i * 60000, 100.0));
    }
    // Spike to trigger Entry
    bars.push(create_bar(symbol, now + 20 * 60000, 110.0));

    let series = BarSeries {
        schema_version: "v0".to_string(),
        bars,
    };

    // 3. Generate Signals
    let intents = signals::generate_signals(&series, "BollingerBands", Some(&history_path)).await.unwrap();

    // 4. Assert Signal Generated
    assert!(!intents.is_empty());
    let intent = &intents[0];

    assert_eq!(intent.symbol, symbol);
    assert!(intent.stop_loss.is_some());
    assert!(intent.take_profit.is_some());

    // Check Rationale includes "No similar past trades found."
    assert!(intent.rationale.contains("No similar past trades found."));
}

#[tokio::test]
async fn test_rag_context() {
    // 1. Setup History with similar trade
    let dir = tempdir().unwrap();
    let history_path = dir.path().join("history.json");
    let symbol = "AAPL";
    let now = 1600000000000;

    // Similar trade
    // We expect "Trending Up" and "High" volatility due to spike
    let history = vec![
        create_history_entry(symbol, now - 86400000 * 10, "Trending Up", "High"),
    ];
    fs::write(&history_path, serde_json::to_string(&history).unwrap()).unwrap();

    // 2. Create BarSeries
    let mut bars = Vec::new();
    for i in 0..20 {
        bars.push(create_bar(symbol, now + i * 60000, 100.0));
    }
    bars.push(create_bar(symbol, now + 20 * 60000, 110.0));

    let series = BarSeries {
        schema_version: "v0".to_string(),
        bars,
    };

    // 3. Generate Signals
    let intents = signals::generate_signals(&series, "BollingerBands", Some(&history_path)).await.unwrap();

    // 4. Assert Context
    assert!(!intents.is_empty());
    let intent = &intents[0];
    assert!(intent.rationale.contains("Found 1 similar past trades."));
}
