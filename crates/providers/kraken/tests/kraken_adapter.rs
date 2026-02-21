use contracts::TradeIntent;
use kraken_provider::{KrakenClient, KrakenConfig};

#[test]
fn from_env_with_requires_required_keys() {
    let cfg = KrakenConfig::from_env_with(|_| None);
    assert!(cfg.is_err());
}

#[test]
fn execute_intent_returns_typed_result() {
    let cfg = KrakenConfig::from_env_with(|key| match key {
        "KRAKEN_API_KEY" => Some("k".to_string()),
        "KRAKEN_API_SECRET" => Some("s".to_string()),
        "KRAKEN_BASE_URL" => Some("https://api.kraken.com".to_string()),
        _ => None,
    })
    .expect("config");
    let client = KrakenClient::new(cfg);

    let result = client
        .execute_intent(&TradeIntent {
            intent_id: "intent-kr-1".to_string(),
            market: "crypto".to_string(),
            symbol: "XBT/USD".to_string(),
            side: "sell".to_string(),
            size_hint: "0.1".to_string(),
            confidence: 0.6,
            horizon: "15m".to_string(),
            rationale: "reversion".to_string(),
            invalidation: "break above range".to_string(),
            schema_version: "v0".to_string(),
        })
        .expect("execution");

    assert_eq!(result.provider, "kraken");
    assert_eq!(result.intent_id, "intent-kr-1");
}
