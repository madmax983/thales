//! Integration tests for the Yahoo provider, using a saved chart API fixture.
//! No network access in tests.

use yahoo_provider::{YahooProviderError, infer_market, parse_chart_response, timeframe_params};

const FIXTURE: &str = include_str!("fixtures/spy_chart_5d.json");

#[test]
fn fixture_parses_to_four_real_bars() {
    // The fixture holds 5 daily timestamps; one bar is all-null (the
    // in-progress session) and must be skipped, not fabricated.
    let bars = parse_chart_response("SPY", "1d", FIXTURE).unwrap();
    assert_eq!(bars.len(), 4);

    let first = &bars[0];
    assert_eq!(first.symbol, "SPY");
    assert_eq!(first.market, "equities");
    assert_eq!(first.timeframe, "1d");
    assert_eq!(first.timestamp_unix_ms, 1789651800 * 1000);
    assert!((first.open - 763.1500244140625).abs() < 1e-9);
    assert!((first.high - 763.5700073242188).abs() < 1e-9);
    assert!((first.low - 759.9600219726562).abs() < 1e-9);
    assert!((first.close - 762.5999755859375).abs() < 1e-9);
    assert!((first.volume - 49652800.0).abs() < 1e-6);

    // Timestamps strictly increasing, already in bar order.
    for pair in bars.windows(2) {
        assert!(pair[0].timestamp_unix_ms < pair[1].timestamp_unix_ms);
    }
}

#[test]
fn null_volume_becomes_zero_not_skipped() {
    // ^VIX-style payload: no volume series at all; bars must survive.
    let body = r#"{"chart":{"result":[{"timestamp":[1700000000,1700086400],"indicators":{"quote":[{"open":[13.5,14.0],"high":[14.2,14.5],"low":[13.1,13.8],"close":[14.0,14.3],"volume":[null,null]}]}}],"error":null}}"#;
    let bars = parse_chart_response("^VIX", "1d", body).unwrap();
    assert_eq!(bars.len(), 2);
    assert_eq!(bars[0].market, "index");
    assert_eq!(bars[0].volume, 0.0);
}

#[test]
fn chart_error_becomes_api_error() {
    let body = r#"{"chart":{"result":null,"error":{"code":"Not Found","description":"No data found, symbol may be delisted"}}}"#;
    let err = parse_chart_response("NOPE", "1d", body).unwrap_err();
    assert!(
        matches!(err, YahooProviderError::Api(_)),
        "expected Api error, got {err:?}"
    );
}

#[test]
fn missing_result_becomes_empty_result() {
    let body = r#"{"chart":{"result":[],"error":null}}"#;
    let err = parse_chart_response("SPY", "1d", body).unwrap_err();
    assert!(
        matches!(err, YahooProviderError::EmptyResult(_)),
        "expected EmptyResult, got {err:?}"
    );
}

#[test]
fn garbage_body_becomes_json_error() {
    let err = parse_chart_response("SPY", "1d", "not json at all").unwrap_err();
    assert!(
        matches!(err, YahooProviderError::Json(_)),
        "expected Json error, got {err:?}"
    );
}

#[test]
fn integration_timeframe_mapping() {
    assert_eq!(
        timeframe_params("1wk").unwrap(),
        ("1wk".to_string(), "2y".to_string())
    );
    assert!(timeframe_params("15m").is_err());
}

#[test]
fn integration_market_inference() {
    assert_eq!(infer_market("NQ=F"), "futures");
    assert_eq!(infer_market("ETH-USD"), "crypto");
}
