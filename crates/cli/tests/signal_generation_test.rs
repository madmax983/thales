use contracts::{Bar, BarSeries, MarketAnalysis, TradeIntent};
use std::fs;
use tempfile::tempdir;
use thales_cli::rag::HistoryEntry;
use thales_cli::signals;

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

fn create_history_entry(
    symbol: &str,
    timestamp: i64,
    regime: &str,
    volatility: &str,
) -> HistoryEntry {
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
            recommendation: None,
        },
        outcome: Some(0.0), // Default to 0.0 (Breakeven/Closed) so it counts as a completed trade
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
    let history_entries = vec![
        create_history_entry(symbol, now - 1000, "Unknown", "Unknown"),
        create_history_entry(symbol, now - 2000, "Unknown", "Unknown"),
        create_history_entry(symbol, now - 3000, "Unknown", "Unknown"),
    ];

    fs::write(
        &history_path,
        serde_json::to_string(&history_entries).unwrap(),
    )
    .unwrap();

    // 2. Create BarSeries that triggers a signal
    let mut bars = Vec::new();
    // 20 bars stable
    for i in 0..20 {
        bars.push(create_bar(symbol, now + i * 60000 + 10000, 100.0));
    }
    // Spike to trigger Entry
    bars.push(create_bar(symbol, now + 20 * 60000 + 10000, 110.0));

    let series = BarSeries {
        schema_version: "v0".to_string(),
        bars,
    };

    // 3. Generate Signals
    let positions = vec![];
    let intents = signals::generate_signals(
        &series,
        "BollingerBands",
        Some(&history_path),
        100.0,
        &positions,
        None,
    )
    .await
    .unwrap();

    // 4. Assert Limit Reached (should be empty because signals_today=3)
    // Actually rag::count_todays_signals might depend on timestamp matching exactly "today"
    // The bars timestamp is `now + ...` which is same day as `now` (1600000000000 is Sep 13 2020)
    // So it should work.

    // Note: The original test expected success or failure depending on the limit logic.
    // If the limit is 3, and we have 3, we expect 0 new signals.
    assert!(
        intents.is_empty(),
        "Should not generate signal if 3 already exist for today"
    );
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
    let positions = vec![];
    let intents = signals::generate_signals(
        &series,
        "BollingerBands",
        Some(&history_path),
        100.0,
        &positions,
        None,
    )
    .await
    .unwrap();

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

    // 2. Create BarSeries first
    let mut bars = Vec::new();
    for i in 0..20 {
        bars.push(create_bar(symbol, now + i * 60000, 100.0));
    }
    bars.push(create_bar(symbol, now + 20 * 60000, 110.0));

    let series = BarSeries {
        schema_version: "v0".to_string(),
        bars,
    };

    // Analyze to get actual regime/volatility
    use thales_cli::analysis;
    let analysis = analysis::analyze(&series);

    // Similar trade matching actual analysis
    let mut entry = create_history_entry(
        symbol,
        now - 86400000 * 10,
        &analysis.regime,
        &analysis.volatility,
    );
    entry.intent.strategy = "BollingerBandsMeanReversion".to_string();

    let history_entries = vec![entry];
    fs::write(
        &history_path,
        serde_json::to_string(&history_entries).unwrap(),
    )
    .unwrap();

    // 3. Generate Signals
    let positions = vec![];
    let intents = signals::generate_signals(
        &series,
        "BollingerBands",
        Some(&history_path),
        100.0,
        &positions,
        None,
    )
    .await
    .unwrap();

    // 4. Assert Context
    assert!(!intents.is_empty());
    let intent = &intents[0];
    assert!(intent.rationale.contains("Found 1 similar past trades"));
}
