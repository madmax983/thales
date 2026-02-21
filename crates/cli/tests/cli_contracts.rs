use std::{fs, process::Command};

use assert_cmd::prelude::*;
use contracts::TradeIntent;
use tempfile::NamedTempFile;

#[test]
fn fetch_market_data_returns_ok_envelope() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("thales-cli"))
        .args([
            "fetch-market-data",
            "--provider",
            "alpaca",
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
    };
    fs::write(
        tmp.path(),
        serde_json::to_string(&intent).expect("intent json"),
    )
    .expect("write");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("thales-cli"))
        .env("ALPACA_API_KEY", "k")
        .env("ALPACA_API_SECRET", "s")
        .env("ALPACA_BASE_URL", "https://paper-api.alpaca.markets")
        .args([
            "execute-intent",
            "--provider",
            "alpaca",
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
    assert_eq!(json["data"]["provider"], "alpaca");
}
