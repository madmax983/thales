use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeIntent {
    pub intent_id: String,
    pub market: String,
    pub symbol: String,
    pub side: String,
    pub size_hint: String,
    pub confidence: f64,
    pub horizon: String,
    pub rationale: String,
    pub invalidation: String,
    pub schema_version: String,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub order_type: String,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub time_in_force: String,
}

impl Default for TradeIntent {
    fn default() -> Self {
        Self {
            intent_id: String::new(),
            market: String::new(),
            symbol: String::new(),
            side: String::new(),
            size_hint: String::new(),
            confidence: 0.0,
            horizon: String::new(),
            rationale: String::new(),
            invalidation: String::new(),
            schema_version: "v0".to_string(),
            stop_loss: None,
            take_profit: None,
            order_type: "market".to_string(),
            limit_price: None,
            stop_price: None,
            time_in_force: "day".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    pub symbol: String,
    pub market: String,
    pub timeframe: String,
    pub timestamp_unix_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarSeries {
    pub schema_version: String,
    pub bars: Vec<Bar>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub schema_version: String,
    pub provider: String,
    pub intent: TradeIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub schema_version: String,
    pub intent_id: String,
    pub provider: String,
    pub provider_order_id: String,
    pub status: String,
    pub submitted_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeStatus {
    Ok,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseEnvelope<T> {
    pub status: EnvelopeStatus,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub data: Option<T>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketAnalysis {
    pub symbol: String,
    pub market: String,
    pub regime: String,
    pub sentiment: String,
    pub patterns: Vec<String>,
    pub key_levels: Vec<f64>,
    pub volatility: String,
    pub confidence: f64,
    pub timestamp_unix_ms: i64,
}
