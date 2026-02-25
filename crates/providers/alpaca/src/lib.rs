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
        validate_side(&intent.side)?;
        if intent.size_hint != "max" {
            validate_size_hint(&intent.size_hint)?;
        }

        let qty = if intent.size_hint == "max" {
            // Fetch positions to find size
            let positions = self.fetch_positions()?;
            let pos = positions.into_iter().find(|p| p.symbol == intent.symbol)
                .ok_or_else(|| AlpacaProviderError::InvalidSizeHint(format!("No open position found for max exit for {}", intent.symbol)))?;
            // We need to return abs value of qty because Alpaca positions can be negative (short)
            // But qty in order must be positive.
            pos.qty.abs().to_string()
        } else {
             intent.size_hint.clone()
        };

        let request = AlpacaOrderRequest {
            symbol: intent.symbol.clone(),
            qty,
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

        if let Some(algo) = &intent.execution_algo {
            // Log that we are using a specific algo (though effectively just passing through to Alpaca as standard order for now)
            // In a real system, this might trigger a specific algo order type if supported.
            eprintln!("Executing with Algo: {}", algo);
        }

        Ok(ExecutionResult {
            schema_version: "v0".to_string(),
            intent_id: intent.intent_id.clone(),
            provider: "alpaca".to_string(),
            provider_order_id: order.id,
            status: order.status,
            submitted_at_unix_ms,
        })
    }

    pub fn fetch_open_orders(&self) -> Result<Vec<contracts::Order>, AlpacaProviderError> {
        let url = format!("{}/v2/orders?status=open", self.config.base_url.trim_end_matches('/'));

        let response = self
            .http
            .get(url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.api_secret)
            .send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(AlpacaProviderError::UnexpectedHttpStatus(status, body));
        }

        let orders: Vec<AlpacaOrder> = response.json()?;
        let mut contract_orders = Vec::new();

        for o in orders {
            let dt = DateTime::parse_from_rfc3339(&o.submitted_at)
                .map_err(|e| AlpacaProviderError::DateParse(e.to_string()))?;

            let qty = o.qty.unwrap_or_else(|| "0".to_string()).parse::<f64>().unwrap_or(0.0);
            let filled_qty = o.filled_qty.parse::<f64>().unwrap_or(0.0);

            contract_orders.push(contracts::Order {
                id: o.id,
                symbol: o.symbol,
                qty,
                filled_qty,
                side: o.side,
                order_type: o.order_type,
                status: o.status,
                submitted_at_unix_ms: dt.timestamp_millis(),
            });
        }

        Ok(contract_orders)
    }

    pub fn cancel_order(&self, order_id: &str) -> Result<(), AlpacaProviderError> {
        let url = format!("{}/v2/orders/{}", self.config.base_url.trim_end_matches('/'), order_id);

        let response = self
            .http
            .delete(url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.api_secret)
            .send()?;

        if !response.status().is_success() {
            // 404 means order not found or already done, which implies it's canceled or filled.
            // But strict API check might want to error. Let's error for now.
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(AlpacaProviderError::UnexpectedHttpStatus(status, body));
        }

        Ok(())
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

    pub fn fetch_positions(&self) -> Result<Vec<AlpacaPosition>, AlpacaProviderError> {
        let url = format!("{}/v2/positions", self.config.base_url.trim_end_matches('/'));

        let response = self
            .http
            .get(url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.api_secret)
            .send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(AlpacaProviderError::UnexpectedHttpStatus(status, body));
        }

        let positions: Vec<AlpacaPosition> = response.json()?;
        Ok(positions)
    }

    pub fn get_open_positions(&self) -> Result<Vec<contracts::Position>, AlpacaProviderError> {
        let positions = self.fetch_positions()?;
        Ok(positions
            .into_iter()
            .map(|p| {
                let cost_basis = p.cost_basis.parse::<f64>().unwrap_or(0.0);
                let entry_price = if p.qty != 0.0 {
                    Some(cost_basis / p.qty)
                } else {
                    None
                };

                contracts::Position {
                    symbol: p.symbol,
                    side: p.side,
                    qty: p.qty,
                    entry_price,
                }
            })
            .collect())
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

#[derive(Debug, Clone, Deserialize)]
struct AlpacaOrder {
    id: String,
    symbol: String,
    qty: Option<String>,
    filled_qty: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    status: String,
    submitted_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlpacaPosition {
    pub symbol: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub qty: f64,
    pub side: String,
    pub market_value: Option<String>,
    pub cost_basis: String,
    pub unrealized_pl: Option<String>,
}

fn deserialize_number_from_string<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<f64>().map_err(serde::de::Error::custom)
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
