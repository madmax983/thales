use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::DateTime;
use contracts::{Bar, ExecutionResult, TradeIntent};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlpacaConfig {
    pub api_key: String,
    pub api_secret: String,
    pub base_url: String,
}

impl AlpacaConfig {
    pub fn from_env() -> Result<Self, AlpacaProviderError> {
        Self::from_env_with(|key| std::env::var(key).ok())
    }

    pub fn from_env_with<F>(get: F) -> Result<Self, AlpacaProviderError>
    where
        F: Fn(&str) -> Option<String>,
    {
        Ok(Self {
            api_key: required_var("ALPACA_API_KEY", &get)?,
            api_secret: required_var("ALPACA_API_SECRET", &get)?,
            base_url: required_var("ALPACA_BASE_URL", &get)?,
        })
    }
}

fn required_var<F>(name: &'static str, get: &F) -> Result<String, AlpacaProviderError>
where
    F: Fn(&str) -> Option<String>,
{
    get(name).ok_or(AlpacaProviderError::MissingEnvVar(name))
}

#[derive(Debug, Clone)]
pub struct AlpacaClient {
    pub config: AlpacaConfig,
    http: Client,
}

impl AlpacaClient {
    pub fn new(config: AlpacaConfig) -> Self {
        Self {
            config,
            http: Client::new(),
        }
    }

    pub fn execute_intent(
        &self,
        intent: &TradeIntent,
    ) -> Result<ExecutionResult, AlpacaProviderError> {
        validate_size_hint(&intent.size_hint)?;
        validate_side(&intent.side)?;

        let request = AlpacaOrderRequest {
            symbol: intent.symbol.clone(),
            qty: intent.size_hint.clone(),
            side: intent.side.clone(),
            order_type: intent.order_type.clone(),
            time_in_force: intent.time_in_force.clone(),
            client_order_id: intent.intent_id.clone(),
            limit_price: intent.limit_price,
            stop_price: intent.stop_price,
            take_profit: intent.take_profit.map(|p| TakeProfitSpec { limit_price: p }),
            stop_loss: intent.stop_loss.map(|p| StopLossSpec {
                stop_price: p,
                limit_price: None,
            }),
        };
        let url = format!("{}/v2/orders", self.config.base_url.trim_end_matches('/'));

        let response = self
            .http
            .post(url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.api_secret)
            .json(&request)
            .send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(AlpacaProviderError::UnexpectedHttpStatus(status, body));
        }

        let order: AlpacaOrderResponse = response.json()?;
        let submitted_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| AlpacaProviderError::Clock(err.to_string()))?
            .as_millis() as i64;

        Ok(ExecutionResult {
            schema_version: "v0".to_string(),
            intent_id: intent.intent_id.clone(),
            provider: "alpaca".to_string(),
            provider_order_id: order.id,
            status: order.status,
            submitted_at_unix_ms,
        })
    }

    pub fn fetch_bars(
        &self,
        symbol: &str,
        timeframe: &str,
    ) -> Result<Vec<Bar>, AlpacaProviderError> {
        let tf = match timeframe {
            "1m" => "1Min",
            "5m" => "5Min",
            "15m" => "15Min",
            "1h" => "1Hour",
            "1d" => "1Day",
            _ => return Err(AlpacaProviderError::InvalidTimeframe(timeframe.to_string())),
        };

        // Hardcoded Data API URL (v2)
        let url = "https://data.alpaca.markets/v2/stocks/bars";
        let params = [
            ("symbols", symbol),
            ("timeframe", tf),
            ("limit", "1000"),
            ("adjustment", "raw"),
        ];

        let response = self
            .http
            .get(url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.api_secret)
            .query(&params)
            .send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(AlpacaProviderError::UnexpectedHttpStatus(status, body));
        }

        let api_response: AlpacaBarsResponse = response.json()?;
        let mut bars = Vec::new();
        if let Some(symbol_bars) = api_response.bars.get(symbol) {
            for b in symbol_bars {
                let dt = DateTime::parse_from_rfc3339(&b.t)
                    .map_err(|e| AlpacaProviderError::DateParse(e.to_string()))?;

                bars.push(Bar {
                    symbol: symbol.to_string(),
                    market: "equities".to_string(),
                    timeframe: timeframe.to_string(),
                    timestamp_unix_ms: dt.timestamp_millis(),
                    open: b.o,
                    high: b.h,
                    low: b.l,
                    close: b.c,
                    volume: b.v as f64,
                });
            }
        }

        // Sort by time
        bars.sort_by_key(|b| b.timestamp_unix_ms);
        Ok(bars)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct AlpacaOrderRequest {
    symbol: String,
    qty: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    time_in_force: String,
    client_order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    take_profit: Option<TakeProfitSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_loss: Option<StopLossSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct TakeProfitSpec {
    limit_price: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct StopLossSpec {
    stop_price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit_price: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct AlpacaOrderResponse {
    id: String,
    status: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AlpacaBarsResponse {
    bars: HashMap<String, Vec<AlpacaBar>>,
    next_page_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AlpacaBar {
    t: String,
    o: f64,
    h: f64,
    l: f64,
    c: f64,
    v: u64,
    n: u64,
    vw: f64,
}

fn validate_size_hint(size_hint: &str) -> Result<(), AlpacaProviderError> {
    let qty = size_hint
        .parse::<f64>()
        .map_err(|_| AlpacaProviderError::InvalidSizeHint(size_hint.to_string()))?;
    if qty <= 0.0 {
        return Err(AlpacaProviderError::InvalidSizeHint(size_hint.to_string()));
    }
    Ok(())
}

fn validate_side(side: &str) -> Result<(), AlpacaProviderError> {
    if side == "buy" || side == "sell" {
        Ok(())
    } else {
        Err(AlpacaProviderError::InvalidSide(side.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum AlpacaProviderError {
    #[error("missing required environment variable: {0}")]
    MissingEnvVar(&'static str),
    #[error("system clock error: {0}")]
    Clock(String),
    #[error("invalid size hint: {0}")]
    InvalidSizeHint(String),
    #[error("invalid side: {0}")]
    InvalidSide(String),
    #[error("http transport error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected alpaca response status {0}: {1}")]
    UnexpectedHttpStatus(u16, String),
    #[error("invalid timeframe: {0}")]
    InvalidTimeframe(String),
    #[error("date parse error: {0}")]
    DateParse(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}
