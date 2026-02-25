use contracts::{
    Bar, BarSeries, EnvelopeStatus, ExecutionRequest, ExecutionResult, ResponseEnvelope,
    TradeIntent,
};

#[test]
fn trade_intent_roundtrip_preserves_v0_schema() {
    let intent = TradeIntent {
        intent_id: "intent-1".to_string(),
        market: "equities".to_string(),
        symbol: "AAPL".to_string(),
        side: "buy".to_string(),
        size_hint: "10".to_string(),
        confidence: 0.75,
        horizon: "1h".to_string(),
        rationale: "simple momentum".to_string(),
        invalidation: "price < 180".to_string(),
        schema_version: "v0".to_string(),
        signal_type: None,
        stop_loss: None,
        take_profit: None,
        order_type: "market".to_string(),
        limit_price: None,
        stop_price: None,
        time_in_force: "day".to_string(),
        execution_algo: None,
    };

    let json = serde_json::to_value(&intent).expect("serialize");
    assert_eq!(json["schema_version"], "v0");

    let decoded: TradeIntent = serde_json::from_value(json).expect("deserialize");
    assert_eq!(decoded.intent_id, "intent-1");
}

#[test]
fn response_envelope_contains_required_fields() {
    let bar = Bar {
        symbol: "AAPL".to_string(),
        market: "equities".to_string(),
        timeframe: "1m".to_string(),
        timestamp_unix_ms: 1_700_000_000_000,
        open: 100.0,
        high: 101.0,
        low: 99.5,
        close: 100.5,
        volume: 10_000.0,
    };
    let series = BarSeries {
        schema_version: "v0".to_string(),
        bars: vec![bar],
    };
    let envelope = ResponseEnvelope {
        status: EnvelopeStatus::Ok,
        errors: vec![],
        warnings: vec!["note".to_string()],
        data: Some(series),
    };

    let json = serde_json::to_value(&envelope).expect("serialize");
    assert!(json.get("status").is_some());
    assert!(json.get("errors").is_some());
    assert!(json.get("warnings").is_some());
    assert!(json.get("data").is_some());
}

#[test]
fn execution_contracts_serialize() {
    let request = ExecutionRequest {
        schema_version: "v0".to_string(),
        provider: "alpaca".to_string(),
        intent: TradeIntent {
            intent_id: "intent-2".to_string(),
            market: "equities".to_string(),
            symbol: "MSFT".to_string(),
            side: "sell".to_string(),
            size_hint: "5".to_string(),
            confidence: 0.55,
            horizon: "15m".to_string(),
            rationale: "mean reversion".to_string(),
            invalidation: "price > 421".to_string(),
            schema_version: "v0".to_string(),
            signal_type: None,
            stop_loss: None,
            take_profit: None,
            order_type: "market".to_string(),
            limit_price: None,
            stop_price: None,
            time_in_force: "day".to_string(),
            execution_algo: None,
        },
    };

    let result = ExecutionResult {
        schema_version: "v0".to_string(),
        intent_id: request.intent.intent_id.clone(),
        provider: request.provider.clone(),
        provider_order_id: "order-1".to_string(),
        status: "submitted".to_string(),
        submitted_at_unix_ms: 1_700_000_100_000,
    };

    let req_json = serde_json::to_string(&request).expect("serialize request");
    let result_json = serde_json::to_string(&result).expect("serialize result");
    assert!(req_json.contains("\"schema_version\":\"v0\""));
    assert!(result_json.contains("\"status\":\"submitted\""));
}
