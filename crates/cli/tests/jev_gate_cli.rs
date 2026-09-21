//! End-to-end coverage of `thales-cli judge-signals` against a mock System One API.

use std::io::Write;

use assert_cmd::Command;
use serde_json::{Value, json};
use tempfile::NamedTempFile;

fn write_json(value: &Value) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp file");
    write!(file, "{value}").expect("write temp file");
    file.flush().expect("flush");
    file
}

fn signal(symbol: &str) -> Value {
    json!([{
        "intent_id": format!("crypto:{symbol}:buy:v0"),
        "market": "crypto",
        "symbol": symbol,
        "side": "buy",
        "size_hint": "2",
        "confidence": 0.7,
        "horizon": "1h",
        "rationale": "Lower band touch",
        "invalidation": "Close below band",
        "schema_version": "v0",
        "order_type": "market",
        "time_in_force": "day",
        "strategy": "BollingerBands"
    }])
}

/// A full answer set, with every knob a test might want to move.
fn answers(decision: &str, p: f64, confidence: f64, instrument_quality: f64) -> String {
    let rest = (1.0 - p) / 2.0;
    json!({
        "model": "jev-1.13.0",
        "answers": {
            "verdict": {
                "type": "choice",
                "choice": decision,
                "confidence": confidence,
                "probabilities": {
                    "execute": if decision == "execute" { p } else { rest },
                    "reduce_size": if decision == "reduce_size" { p } else { rest },
                    "skip": if decision == "skip" { p } else { rest },
                }
            },
            "instrument_quality": {"type": "noul", "noul": instrument_quality},
            "regime_fit": {"type": "noul", "noul": 0.82},
            "conviction": {
                "type": "score",
                "score": 3.0,
                "confidence": 0.9,
                "legend": {"0": "No edge", "1": "Weak", "2": "Fair", "3": "Strong", "4": "Exceptional"},
                "probabilities": {"0": 0.0, "1": 0.1, "2": 0.2, "3": 0.6, "4": 0.1}
            }
        },
        "usage": {"input_tokens": 410, "output_tokens": 14}
    })
    .to_string()
}

fn run(server: &mockito::Server, args: &[&str]) -> (Value, bool) {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("thales-cli"))
        .env("TYPESAFE_API_KEY", "sk-test")
        .env("TYPESAFE_BASE_URL", server.url())
        .args(args)
        .output()
        .expect("run cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout was not JSON ({e}): {stdout}"));
    (parsed, output.status.success())
}

#[test]
fn an_approved_signal_is_emitted_ready_for_execute_intent() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v1/systemone")
        .with_status(200)
        .with_body(answers("execute", 0.84, 0.91, 0.97))
        .create();

    let input = write_json(&signal("BTCUSD"));
    let (envelope, success) = run(
        &server,
        &["judge-signals", "--input", input.path().to_str().unwrap()],
    );

    assert!(success);
    assert_eq!(envelope["status"], "ok");

    let intents = envelope["data"].as_array().expect("data is a list");
    assert_eq!(intents.len(), 1);
    // The calibrated probability replaces the strategy's own confidence.
    assert_eq!(intents[0]["confidence"], 0.84);
    assert_eq!(intents[0]["symbol"], "BTCUSD");
    assert_eq!(intents[0]["size_hint"], "2");
    assert!(
        intents[0]["rationale"]
            .as_str()
            .unwrap()
            .contains("jev[jev-1.13.0]")
    );
}

#[test]
fn the_emitted_shape_is_what_execute_intent_consumes() {
    // judge-signals is only useful if its output pipes onward unchanged, so
    // assert the contract rather than trusting it.
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v1/systemone")
        .with_status(200)
        .with_body(answers("execute", 0.84, 0.91, 0.97))
        .create();

    let input = write_json(&signal("BTCUSD"));
    let (envelope, _) = run(
        &server,
        &["judge-signals", "--input", input.path().to_str().unwrap()],
    );

    let judged = write_json(&envelope);
    let output = Command::new(assert_cmd::cargo::cargo_bin!("thales-cli"))
        .args([
            "execute-intent",
            "--provider",
            "paper",
            "--input",
            judged.path().to_str().unwrap(),
        ])
        .output()
        .expect("run cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    assert_eq!(
        parsed["status"], "ok",
        "execute-intent rejected the output: {stdout}"
    );
}

#[test]
fn a_thin_novelty_listing_is_vetoed_end_to_end() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v1/systemone")
        .with_status(200)
        // A textbook-looking setup on an instrument that should not be traded.
        .with_body(answers("execute", 0.96, 0.98, 0.04))
        .create();

    let input = write_json(&signal("APENFT"));
    let (envelope, success) = run(
        &server,
        &["judge-signals", "--input", input.path().to_str().unwrap()],
    );

    assert!(success);
    assert!(envelope["data"].as_array().unwrap().is_empty());
    let warnings = envelope["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("instrument quality 0.04")),
        "warnings: {warnings:?}"
    );
}

#[test]
fn reduce_size_verdict_halves_the_position() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v1/systemone")
        .with_status(200)
        .with_body(answers("reduce_size", 0.72, 0.88, 0.95))
        .create();

    let input = write_json(&signal("ETHUSD"));
    let (envelope, _) = run(
        &server,
        &["judge-signals", "--input", input.path().to_str().unwrap()],
    );

    let intents = envelope["data"].as_array().unwrap();
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0]["size_hint"], "1");
}

#[test]
fn emit_report_returns_the_full_audit_trail() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v1/systemone")
        .with_status(200)
        .with_body(answers("execute", 0.84, 0.91, 0.97))
        .create();

    let input = write_json(&signal("BTCUSD"));
    let (envelope, _) = run(
        &server,
        &[
            "judge-signals",
            "--input",
            input.path().to_str().unwrap(),
            "--emit",
            "report",
        ],
    );

    let report = &envelope["data"];
    assert_eq!(report["approved"].as_array().unwrap().len(), 1);
    assert_eq!(report["rejected"].as_array().unwrap().len(), 0);

    let verdict = &report["verdicts"][0];
    assert_eq!(verdict["decision"], "execute");
    assert_eq!(verdict["approved"], true);
    assert_eq!(verdict["strategy_confidence"], 0.7);
    assert_eq!(verdict["execute_probability"], 0.84);
    assert_eq!(verdict["instrument_quality"], 0.97);
    assert_eq!(verdict["regime_fit"], 0.82);
    assert_eq!(verdict["conviction"]["label"], "Strong");
    assert_eq!(report["usage"]["input_tokens"], 410);
    assert_eq!(report["thresholds"]["min_instrument_quality"], 0.5);
}

#[test]
fn rejections_are_appended_to_the_audit_log() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v1/systemone")
        .with_status(200)
        .with_body(answers("skip", 0.9, 0.95, 0.99))
        .create();

    let input = write_json(&signal("KOBAN"));
    let log = NamedTempFile::new().expect("temp file");

    let (_, success) = run(
        &server,
        &[
            "judge-signals",
            "--input",
            input.path().to_str().unwrap(),
            "--log",
            log.path().to_str().unwrap(),
        ],
    );
    assert!(success);

    let written = std::fs::read_to_string(log.path()).expect("read log");
    assert!(written.contains("| Date/Time | Symbol | Signal Ref | Rejection Reason |"));
    assert!(written.contains("KOBAN"));
    assert!(written.contains("crypto:KOBAN:buy:v0"));
}

#[test]
fn a_missing_api_key_fails_loudly_rather_than_passing_signals_through() {
    let input = write_json(&signal("BTCUSD"));
    let output = Command::new(assert_cmd::cargo::cargo_bin!("thales-cli"))
        .env_remove("TYPESAFE_API_KEY")
        .args(["judge-signals", "--input", input.path().to_str().unwrap()])
        .output()
        .expect("run cli");

    assert!(!output.status.success(), "must not exit 0 without a key");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: Value = serde_json::from_str(stdout.trim()).expect("json");
    assert_eq!(envelope["status"], "error");
    assert!(
        envelope["errors"][0]
            .as_str()
            .unwrap()
            .contains("TYPESAFE_API_KEY")
    );
}

#[test]
fn an_upstream_failure_is_surfaced_not_swallowed() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v1/systemone")
        .with_status(401)
        .create();

    let input = write_json(&signal("BTCUSD"));
    let (envelope, success) = run(
        &server,
        &["judge-signals", "--input", input.path().to_str().unwrap()],
    );

    assert!(!success);
    assert_eq!(envelope["status"], "error");
}

#[test]
fn invalid_thresholds_are_rejected_before_any_request() {
    let mut server = mockito::Server::new();
    let mock = server.mock("POST", "/v1/systemone").expect(0).create();

    let input = write_json(&signal("BTCUSD"));
    let (envelope, success) = run(
        &server,
        &[
            "judge-signals",
            "--input",
            input.path().to_str().unwrap(),
            "--min-probability",
            "1.5",
        ],
    );

    mock.assert();
    assert!(!success);
    assert_eq!(envelope["status"], "error");
}

#[test]
fn an_unknown_emit_mode_is_rejected() {
    let mut server = mockito::Server::new();
    let mock = server.mock("POST", "/v1/systemone").expect(0).create();

    let input = write_json(&signal("BTCUSD"));
    let (_, success) = run(
        &server,
        &[
            "judge-signals",
            "--input",
            input.path().to_str().unwrap(),
            "--emit",
            "nonsense",
        ],
    );

    mock.assert();
    assert!(!success);
}

#[test]
fn analysis_and_bars_are_folded_into_the_state() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/v1/systemone")
        .match_body(mockito::Matcher::PartialJson(json!({
            "state": {
                "market_analysis": {"regime": "Trending Up"},
                "recent_price_action": {"timeframe": "1h"}
            }
        })))
        .with_status(200)
        .with_body(answers("execute", 0.84, 0.91, 0.97))
        .create();

    let analysis = write_json(&json!({
        "symbol": "BTCUSD", "market": "crypto", "regime": "Trending Up",
        "sentiment": "Bullish", "patterns": [], "key_levels": [60000.0],
        "volatility": "High", "atr": 1200.0, "confidence": 0.8,
        "timestamp_unix_ms": 0
    }));
    let bars = write_json(&json!({
        "schema_version": "v0",
        "bars": [{
            "symbol": "BTCUSD", "market": "crypto", "timeframe": "1h",
            "timestamp_unix_ms": 1, "open": 100.0, "high": 110.0,
            "low": 95.0, "close": 108.0, "volume": 50.0
        }]
    }));
    let input = write_json(&signal("BTCUSD"));

    let (_, success) = run(
        &server,
        &[
            "judge-signals",
            "--input",
            input.path().to_str().unwrap(),
            "--analysis",
            analysis.path().to_str().unwrap(),
            "--bars",
            bars.path().to_str().unwrap(),
        ],
    );

    mock.assert();
    assert!(success);
}

// --- analyze-market --jev ---------------------------------------------------

const CLASSIFICATION: &str = r#"{
  "model": "jev-1.13.0",
  "answers": {
    "regime": {
      "type": "choice", "choice": "Trending Up", "confidence": 0.9,
      "probabilities": {"Trending Up": 0.77, "Ranging": 0.13, "Volatile": 0.10}
    },
    "sentiment": {
      "type": "choice", "choice": "Bullish", "confidence": 0.86,
      "probabilities": {"Bullish": 0.74, "Neutral": 0.16, "Bearish": 0.10}
    },
    "volatility": {
      "type": "choice", "choice": "High", "confidence": 0.81,
      "probabilities": {"High": 0.68, "Normal": 0.22, "Low": 0.10}
    }
  },
  "usage": {"input_tokens": 260, "output_tokens": 11}
}"#;

fn bars() -> Value {
    json!({
        "schema_version": "v0",
        "bars": (0..30).map(|i| json!({
            "symbol": "BTCUSD", "market": "crypto", "timeframe": "1h",
            "timestamp_unix_ms": 1_600_000_000_000i64 + i * 3_600_000,
            "open": 100.0 + i as f64, "high": 105.0 + i as f64,
            "low": 95.0 + i as f64, "close": 103.0 + i as f64,
            "volume": 1000.0 + i as f64
        })).collect::<Vec<_>>()
    })
}

#[test]
fn analyze_market_jev_replaces_heuristic_labels_and_keeps_the_distribution() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v1/systemone")
        .with_status(200)
        .with_body(CLASSIFICATION)
        .create();

    let input = write_json(&bars());
    let (envelope, success) = run(
        &server,
        &[
            "analyze-market",
            "--input",
            input.path().to_str().unwrap(),
            "--no-report",
            "--jev",
        ],
    );

    assert!(success);
    let data = &envelope["data"];
    assert_eq!(data["regime"], "Trending Up");
    assert_eq!(data["sentiment"], "Bullish");
    assert_eq!(data["volatility"], "High");
    // Overall confidence becomes the probability behind the regime call.
    assert_eq!(data["confidence"], 0.77);
    assert_eq!(data["jev"]["model"], "jev-1.13.0");
    assert_eq!(data["jev"]["regime"]["probabilities"]["Ranging"], 0.13);
    assert_eq!(data["jev"]["volatility"]["confidence"], 0.81);
}

#[test]
fn analyze_market_without_the_flag_makes_no_request_and_attaches_nothing() {
    let mut server = mockito::Server::new();
    let mock = server.mock("POST", "/v1/systemone").expect(0).create();

    let input = write_json(&bars());
    let (envelope, success) = run(
        &server,
        &[
            "analyze-market",
            "--input",
            input.path().to_str().unwrap(),
            "--no-report",
        ],
    );

    mock.assert();
    assert!(success);
    assert!(envelope["data"].get("jev").is_none());
}

#[test]
fn a_classified_analysis_feeds_straight_back_into_judge_signals() {
    // The two integration points have to compose, or neither is much use.
    let mut server = mockito::Server::new();
    let _classify = server
        .mock("POST", "/v1/systemone")
        .with_status(200)
        .with_body(CLASSIFICATION)
        .expect(1)
        .create();

    let bars_file = write_json(&bars());
    let (analysis_envelope, _) = run(
        &server,
        &[
            "analyze-market",
            "--input",
            bars_file.path().to_str().unwrap(),
            "--no-report",
            "--jev",
        ],
    );
    let analysis_file = write_json(&analysis_envelope);

    let mut gate_server = mockito::Server::new();
    let gate = gate_server
        .mock("POST", "/v1/systemone")
        .match_body(mockito::Matcher::PartialJson(json!({
            "state": {"market_analysis": {"regime": "Trending Up"}}
        })))
        .with_status(200)
        .with_body(answers("execute", 0.84, 0.91, 0.97))
        .create();

    let input = write_json(&signal("BTCUSD"));
    let (envelope, success) = run(
        &gate_server,
        &[
            "judge-signals",
            "--input",
            input.path().to_str().unwrap(),
            "--analysis",
            analysis_file.path().to_str().unwrap(),
        ],
    );

    gate.assert();
    assert!(success);
    assert_eq!(envelope["data"].as_array().unwrap().len(), 1);
}
