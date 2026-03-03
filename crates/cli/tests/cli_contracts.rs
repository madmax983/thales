use std::{fs, process::Command};

use assert_cmd::prelude::*;
use contracts::TradeIntent;
use mockito::Matcher;
use tempfile::NamedTempFile;

#[test]
fn fetch_market_data_returns_ok_envelope() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("thales-cli"))
        .args([
            "fetch-market-data",
            "--provider",
            "paper",
            "--symbol",
            "AAPL",
            "--timeframe",
            "1m",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let body = String::from_utf8(output).expect("utf8");
    let json: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(json["status"], "ok");
    assert!(json["data"]["bars"].is_array());
}

#[test]
fn generate_trade_intent_returns_v0_intent() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("thales-cli"))
        .args([
            "generate-trade-intent",
            "--market",
            "equities",
            "--symbol",
            "MSFT",
            "--side",
            "buy",
            "--size-hint",
            "5",
            "--confidence",
            "0.7",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let body = String::from_utf8(output).expect("utf8");
    let json: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"]["schema_version"], "v0");
}

#[test]
fn validate_intent_rejects_invalid_payload() {
    let tmp = NamedTempFile::new().expect("temp file");
    let intent = TradeIntent {
        intent_id: "bad-intent".to_string(),
        market: "equities".to_string(),
        symbol: String::new(),
        side: "buy".to_string(),
        size_hint: "1".to_string(),
        confidence: 1.2,
        horizon: "1h".to_string(),
        rationale: "bad payload".to_string(),
        invalidation: "none".to_string(),
        schema_version: "v0".to_string(),
        signal_type: None,
        stop_loss: None,
        take_profit: None,
        order_type: "market".to_string(),
        limit_price: None,
        stop_price: None,
        time_in_force: "gtc".to_string(),
        execution_algo: None,
        strategy: "manual".to_string(),
    };
    fs::write(
        tmp.path(),
        serde_json::to_string(&intent).expect("intent json"),
    )
    .expect("write");

    Command::new(assert_cmd::cargo::cargo_bin!("thales-cli"))
        .args([
            "validate-intent",
            "--input",
            tmp.path().to_str().expect("path"),
        ])
        .assert()
        .failure();
}

#[test]
fn execute_intent_returns_provider_result() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/v2/orders")
        .match_header("APCA-API-KEY-ID", "k")
        .match_header("APCA-API-SECRET-KEY", "s")
        .match_body(Matcher::Regex("\"symbol\":\"AAPL\"".to_string()))
        .match_body(Matcher::Regex("\"qty\":\"1\"".to_string()))
        .match_body(Matcher::Regex("\"side\":\"buy\"".to_string()))
        .match_body(Matcher::Regex(
            "\"client_order_id\":\"intent-exec-1\"".to_string(),
        ))
        .with_status(200)
        .with_body(
            serde_json::json!({
                "id": "order-cli-1",
                "status": "accepted"
            })
            .to_string(),
        )
        .create();

    let tmp = NamedTempFile::new().expect("temp file");
    let intent = TradeIntent {
        intent_id: "intent-exec-1".to_string(),
        market: "equities".to_string(),
        symbol: "AAPL".to_string(),
        side: "buy".to_string(),
        size_hint: "1".to_string(),
        confidence: 0.8,
        horizon: "1h".to_string(),
        rationale: "test".to_string(),
        invalidation: "none".to_string(),
        schema_version: "v0".to_string(),
        signal_type: None,
        stop_loss: None,
        take_profit: None,
        order_type: "market".to_string(),
        limit_price: None,
        stop_price: None,
        time_in_force: "gtc".to_string(),
        execution_algo: None,
        strategy: "manual".to_string(),
    };
    fs::write(
        tmp.path(),
        serde_json::to_string(&intent).expect("intent json"),
    )
    .expect("write");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("thales-cli"))
        .env("ALPACA_API_KEY", "k")
        .env("ALPACA_API_SECRET", "s")
        .env("ALPACA_BASE_URL", server.url())
        .args([
            "execute-intent",
            "--provider",
            "paper",
            "--input",
            tmp.path().to_str().expect("path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let body = String::from_utf8(output).expect("utf8");
    let json: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"][0]["provider"], "paper");
    assert_eq!(json["data"][0]["provider_order_id"], "order-cli-1");
    mock.assert();
}

#[test]
fn execute_intent_returns_kraken_provider_result() {
    let mut server = mockito::Server::new();
    let _asset_mock = server
        .mock("GET", "/0/public/AssetPairs?pair=XBTUSD")
        .with_status(200)
        .with_body(
            serde_json::json!({
                "error": [],
                "result": {
                    "XXBTZUSD": { "pair_decimals": 1, "lot_decimals": 8 }
                }
            })
            .to_string(),
        )
        .create();

    let mock = server
        .mock("POST", "/0/private/AddOrder")
        .match_header("API-Key", "k")
        .match_header("API-Sign", Matcher::Regex(".+".to_string()))
        .match_body(Matcher::Regex("pair=XBTUSD".to_string()))
        .match_body(Matcher::Regex("type=sell".to_string()))
        .match_body(Matcher::Regex("volume=0.1".to_string()))
        .with_status(200)
        .with_body(
            serde_json::json!({
                "error": [],
                "result": {
                    "txid": ["kraken-cli-1"]
                }
            })
            .to_string(),
        )
        .create();

    let tmp = NamedTempFile::new().expect("temp file");
    let intent = TradeIntent {
        intent_id: "intent-kr-cli-1".to_string(),
        market: "crypto".to_string(),
        symbol: "XBT/USD".to_string(),
        side: "sell".to_string(),
        size_hint: "0.1".to_string(),
        confidence: 0.8,
        horizon: "1h".to_string(),
        rationale: "test".to_string(),
        invalidation: "none".to_string(),
        schema_version: "v0".to_string(),
        signal_type: None,
        stop_loss: None,
        take_profit: None,
        order_type: "market".to_string(),
        limit_price: None,
        stop_price: None,
        time_in_force: "gtc".to_string(),
        execution_algo: None,
        strategy: "manual".to_string(),
    };
    fs::write(
        tmp.path(),
        serde_json::to_string(&intent).expect("intent json"),
    )
    .expect("write");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("thales-cli"))
        .env("KRAKEN_API_KEY", "k")
        .env("KRAKEN_API_SECRET", "YWJj")
        .env("KRAKEN_BASE_URL", server.url())
        .args([
            "execute-intent",
            "--provider",
            "kraken",
            "--input",
            tmp.path().to_str().expect("path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let body = String::from_utf8(output).expect("utf8");
    let json: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"][0]["provider"], "kraken");
    assert_eq!(json["data"][0]["provider_order_id"], "kraken-cli-1");
    mock.assert();
}
