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
    let _asset_mock = server
        .mock("GET", "/0/public/AssetPairs?pair=XBTUSD")
        .with_status(200)
        .with_body(
            json!({
                "error": [],
                "result": {
                    "XXBTZUSD": { "pair_decimals": 0, "lot_decimals": 8 }
                }
            })
            .to_string(),
        )
        .create();

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
            signal_type: None,
            stop_loss: None,
            take_profit: None,
            order_type: "market".to_string(),
            limit_price: None,
            stop_price: None,
            time_in_force: "gtc".to_string(),
            execution_algo: None,
            strategy: "test_strategy".to_string(),
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
    let _asset_mock = server
        .mock("GET", "/0/public/AssetPairs?pair=XBTUSD")
        .with_status(200)
        .with_body(
            json!({
                "error": [],
                "result": {
                    "XXBTZUSD": { "pair_decimals": 0, "lot_decimals": 8 }
                }
            })
            .to_string(),
        )
        .create();

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
            signal_type: None,
            stop_loss: None,
            take_profit: None,
            order_type: "market".to_string(),
            limit_price: None,
            stop_price: None,
            time_in_force: "gtc".to_string(),
            execution_algo: None,
            strategy: "test_strategy".to_string(),
        })
        .expect_err("expected kraken API error");

    let message = err.to_string();
    assert!(message.contains("kraken api error"));
    mock.assert();
}

#[test]
fn execute_intent_returns_error_for_day_tif() {
    let mut server = mockito::Server::new();
    let _asset_mock = server
        .mock("GET", "/0/public/AssetPairs?pair=XBTUSD")
        .with_status(200)
        .with_body(
            json!({
                "error": [],
                "result": {
                    "XXBTZUSD": { "pair_decimals": 0, "lot_decimals": 8 }
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

    let err = client
        .execute_intent(&TradeIntent {
            intent_id: "intent-day-tif".to_string(),
            market: "crypto".to_string(),
            symbol: "XBT/USD".to_string(),
            side: "sell".to_string(),
            size_hint: "0.1".to_string(),
            confidence: 0.6,
            horizon: "15m".to_string(),
            rationale: "test".to_string(),
            invalidation: "n/a".to_string(),
            schema_version: "v0".to_string(),
            signal_type: None,
            stop_loss: None,
            take_profit: None,
            order_type: "market".to_string(),
            limit_price: None,
            stop_price: None,
            time_in_force: "day".to_string(),
            execution_algo: None,
            strategy: "test_strategy".to_string(),
        })
        .expect_err("expected error");

    assert!(err.to_string().contains("DAY time-in-force not supported"));
}

#[test]
fn execute_intent_submits_limit_order() {
    let mut server = mockito::Server::new();
    let _asset_mock = server
        .mock("GET", "/0/public/AssetPairs?pair=XBTUSD")
        .with_status(200)
        .with_body(
            json!({
                "error": [],
                "result": {
                    "XXBTZUSD": { "pair_decimals": 0, "lot_decimals": 8 }
                }
            })
            .to_string(),
        )
        .create();

    let mock = server
        .mock("POST", "/0/private/AddOrder")
        .match_body(Matcher::Regex("ordertype=limit".to_string()))
        .match_body(Matcher::Regex("price=50000".to_string()))
        .with_status(200)
        .with_body(
            json!({
                "error": [],
                "result": { "txid": ["tx1"] }
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

    client
        .execute_intent(&TradeIntent {
            intent_id: "intent-limit".to_string(),
            market: "crypto".to_string(),
            symbol: "XBT/USD".to_string(),
            side: "buy".to_string(),
            size_hint: "0.1".to_string(),
            confidence: 0.6,
            horizon: "15m".to_string(),
            rationale: "limit".to_string(),
            invalidation: "n/a".to_string(),
            schema_version: "v0".to_string(),
            signal_type: None,
            stop_loss: None,
            take_profit: None,
            order_type: "limit".to_string(),
            limit_price: Some(50000.0),
            stop_price: None,
            time_in_force: "gtc".to_string(),
            execution_algo: None,
            strategy: "test_strategy".to_string(),
        })
        .expect("execution");

    mock.assert();
}

#[test]
fn execute_intent_submits_stop_loss_order_with_close() {
    let mut server = mockito::Server::new();
    let _asset_mock = server
        .mock("GET", "/0/public/AssetPairs?pair=XBTUSD")
        .with_status(200)
        .with_body(
            json!({
                "error": [],
                "result": {
                    "XXBTZUSD": { "pair_decimals": 0, "lot_decimals": 8 }
                }
            })
            .to_string(),
        )
        .create();

    let mock = server
        .mock("POST", "/0/private/AddOrder")
        .match_body(Matcher::Regex("ordertype=market".to_string()))
        .match_body(Matcher::Regex("close\\[ordertype\\]=stop-loss".to_string()))
        .match_body(Matcher::Regex("close\\[price\\]=49000".to_string()))
        .with_status(200)
        .with_body(
            json!({
                "error": [],
                "result": { "txid": ["tx2"] }
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

    client
        .execute_intent(&TradeIntent {
            intent_id: "intent-sl".to_string(),
            market: "crypto".to_string(),
            symbol: "XBT/USD".to_string(),
            side: "buy".to_string(),
            size_hint: "0.1".to_string(),
            confidence: 0.6,
            horizon: "15m".to_string(),
            rationale: "sl".to_string(),
            invalidation: "n/a".to_string(),
            schema_version: "v0".to_string(),
            signal_type: None,
            stop_loss: Some(49000.0),
            take_profit: None,
            order_type: "market".to_string(),
            limit_price: None,
            stop_price: None,
            time_in_force: "gtc".to_string(),
            execution_algo: None,
            strategy: "test_strategy".to_string(),
        })
        .expect("execution");

    mock.assert();
}

#[test]
fn get_buying_power_for_symbol_reads_quote_balance() {
    let mut server = mockito::Server::new();
    let balance_mock = server
        .mock("POST", "/0/private/Balance")
        .match_header("API-Key", "k")
        .match_header("API-Sign", Matcher::Regex(".+".to_string()))
        .match_body(Matcher::Regex("nonce=\\d+".to_string()))
        .with_status(200)
        .with_body(
            json!({
                "error": [],
                "result": {
                    "ZUSD": "250.75"
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

    let (currency, amount) = client
        .get_buying_power_for_symbol("XBT/USD")
        .expect("buying power");
    assert_eq!(currency, "USD");
    assert!((amount - 250.75).abs() < 1e-6);
    balance_mock.assert();
}
