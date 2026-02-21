use alpaca_provider::{AlpacaClient, AlpacaConfig};
use contracts::TradeIntent;
use mockito::Matcher;
use serde_json::json;

#[test]
fn from_env_with_requires_required_keys() {
    let cfg = AlpacaConfig::from_env_with(|_| None);
    assert!(cfg.is_err());
}

#[test]
fn execute_intent_submits_order_and_maps_response() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/v2/orders")
        .match_header("APCA-API-KEY-ID", "k")
        .match_header("APCA-API-SECRET-KEY", "s")
        .match_body(Matcher::Regex("\"symbol\":\"AAPL\"".to_string()))
        .match_body(Matcher::Regex("\"qty\":\"1\"".to_string()))
        .match_body(Matcher::Regex("\"side\":\"buy\"".to_string()))
        .match_body(Matcher::Regex(
            "\"client_order_id\":\"intent-123\"".to_string(),
        ))
        .with_status(200)
        .with_body(
            json!({
                "id": "order-123",
                "status": "accepted"
            })
            .to_string(),
        )
        .create();

    let cfg = AlpacaConfig::from_env_with(|key| match key {
        "ALPACA_API_KEY" => Some("k".to_string()),
        "ALPACA_API_SECRET" => Some("s".to_string()),
        "ALPACA_BASE_URL" => Some(server.url()),
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
    assert_eq!(result.provider_order_id, "order-123");
    assert_eq!(result.status, "accepted");
    mock.assert();
}

#[test]
fn execute_intent_returns_error_on_http_failure() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/v2/orders")
        .with_status(422)
        .with_body(
            json!({
                "message": "unprocessable entity"
            })
            .to_string(),
        )
        .create();

    let cfg = AlpacaConfig::from_env_with(|key| match key {
        "ALPACA_API_KEY" => Some("k".to_string()),
        "ALPACA_API_SECRET" => Some("s".to_string()),
        "ALPACA_BASE_URL" => Some(server.url()),
        _ => None,
    })
    .expect("config");
    let client = AlpacaClient::new(cfg);

    let err = client
        .execute_intent(&TradeIntent {
            intent_id: "intent-err".to_string(),
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
        .expect_err("expected http error");

    let message = err.to_string();
    assert!(message.contains("unexpected alpaca response status"));
    mock.assert();
}
