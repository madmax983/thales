use alpaca_provider::{AlpacaClient, AlpacaConfig};
use contracts::TradeIntent;

#[test]
fn from_env_with_requires_required_keys() {
    let cfg = AlpacaConfig::from_env_with(|_| None);
    assert!(cfg.is_err());
}

#[test]
fn execute_intent_returns_typed_result() {
    let cfg = AlpacaConfig::from_env_with(|key| match key {
        "ALPACA_API_KEY" => Some("k".to_string()),
        "ALPACA_API_SECRET" => Some("s".to_string()),
        "ALPACA_BASE_URL" => Some("https://paper-api.alpaca.markets".to_string()),
        _ => None,
    })
    .expect("config");
    let client = AlpacaClient::new(cfg);

    let result = client
        .execute_intent(&TradeIntent {
            intent_id: "intent-123".to_string(),
            market: "equities".to_string(),
            symbol: "AAPL".to_string(),
            side: "buy".to_string(),
            size_hint: "1".to_string(),
            confidence: 0.9,
            horizon: "1h".to_string(),
            rationale: "breakout".to_string(),
            invalidation: "below support".to_string(),
            schema_version: "v0".to_string(),
        })
        .expect("execution");

    assert_eq!(result.provider, "alpaca");
    assert_eq!(result.intent_id, "intent-123");
}
