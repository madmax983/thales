use contracts::TradeIntent;

#[test]
fn contracts_smoke_roundtrip() {
    let json = serde_json::to_string(&TradeIntent::default()).expect("serialize");
    assert!(json.contains("schema_version"));
}
