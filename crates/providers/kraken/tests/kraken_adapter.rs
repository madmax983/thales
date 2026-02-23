use contracts::TradeIntent;
use kraken_provider::{KrakenClient, KrakenConfig};
use mockito::Matcher;
use serde_json::json;

#[test]
fn from_env_with_requires_required_keys() {
    let cfg = KrakenConfig::from_env_with(|_| None);
    assert!(cfg.is_err());
}

#[test]
fn execute_intent_submits_order_and_maps_response() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/0/private/AddOrder")
        .match_header("API-Key", "k")
        .match_header("API-Sign", Matcher::Regex(".+".to_string()))
        .match_header(
            "content-type",
            Matcher::Regex("application/x-www-form-urlencoded".to_string()),
        )
        .match_body(Matcher::Regex("pair=XBTUSD".to_string()))
        .match_body(Matcher::Regex("type=sell".to_string()))
        .match_body(Matcher::Regex("ordertype=market".to_string()))
        .match_body(Matcher::Regex("volume=0.1".to_string()))
        .match_body(Matcher::Regex("nonce=\\d+".to_string()))
        .with_status(200)
        .with_body(
            json!({
                "error": [],
                "result": {
                    "txid": ["kraken-order-1"]
                }
            })
            .to_string(),
        )
        .create();

    let cfg = KrakenConfig::from_env_with(|key| match key {
        "KRAKEN_API_KEY" => Some("k".to_string()),
        "KRAKEN_API_SECRET" => Some("YWJj".to_string()),
        "KRAKEN_BASE_URL" => Some(server.url()),
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
            stop_loss: None,
            take_profit: None,
        })
        .expect("execution");

    assert_eq!(result.provider, "kraken");
    assert_eq!(result.intent_id, "intent-kr-1");
    assert_eq!(result.provider_order_id, "kraken-order-1");
    assert_eq!(result.status, "submitted");
    mock.assert();
}

#[test]
fn execute_intent_returns_error_when_kraken_error_array_is_non_empty() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/0/private/AddOrder")
        .with_status(200)
        .with_body(
            json!({
                "error": ["EGeneral:Invalid arguments"],
                "result": null
            })
            .to_string(),
        )
        .create();

    let cfg = KrakenConfig::from_env_with(|key| match key {
        "KRAKEN_API_KEY" => Some("k".to_string()),
        "KRAKEN_API_SECRET" => Some("YWJj".to_string()),
        "KRAKEN_BASE_URL" => Some(server.url()),
        _ => None,
    })
    .expect("config");
    let client = KrakenClient::new(cfg);

    let err = client
        .execute_intent(&TradeIntent {
            intent_id: "intent-kr-err".to_string(),
            market: "crypto".to_string(),
            symbol: "XBT/USD".to_string(),
            side: "sell".to_string(),
            size_hint: "0.1".to_string(),
            confidence: 0.6,
            horizon: "15m".to_string(),
            rationale: "reversion".to_string(),
            invalidation: "break above range".to_string(),
            schema_version: "v0".to_string(),
            stop_loss: None,
            take_profit: None,
        })
        .expect_err("expected kraken API error");

    let message = err.to_string();
    assert!(message.contains("kraken api error"));
    mock.assert();
}
