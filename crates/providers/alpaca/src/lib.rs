use std::time::{SystemTime, UNIX_EPOCH};

use contracts::{ExecutionResult, TradeIntent};
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
            order_type: "market".to_string(),
            time_in_force: "day".to_string(),
            client_order_id: intent.intent_id.clone(),
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct AlpacaOrderRequest {
    symbol: String,
    qty: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    time_in_force: String,
    client_order_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct AlpacaOrderResponse {
    id: String,
    status: String,
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
}
