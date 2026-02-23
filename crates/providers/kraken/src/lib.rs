use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::STANDARD};
use contracts::{ExecutionResult, TradeIntent};
use hmac::{Hmac, Mac};
use reqwest::blocking::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256, Sha512};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KrakenConfig {
    pub api_key: String,
    pub api_secret: String,
    pub base_url: String,
}

impl KrakenConfig {
    pub fn from_env() -> Result<Self, KrakenProviderError> {
        Self::from_env_with(|key| std::env::var(key).ok())
    }

    pub fn from_env_with<F>(get: F) -> Result<Self, KrakenProviderError>
    where
        F: Fn(&str) -> Option<String>,
    {
        Ok(Self {
            api_key: required_var("KRAKEN_API_KEY", &get)?,
            api_secret: required_var("KRAKEN_API_SECRET", &get)?,
            base_url: get("KRAKEN_BASE_URL")
                .unwrap_or_else(|| "https://api.kraken.com".to_string()),
        })
    }
}

fn required_var<F>(name: &'static str, get: &F) -> Result<String, KrakenProviderError>
where
    F: Fn(&str) -> Option<String>,
{
    get(name).ok_or(KrakenProviderError::MissingEnvVar(name))
}

#[derive(Debug, Clone)]
pub struct KrakenClient {
    pub config: KrakenConfig,
    http: Client,
}

impl KrakenClient {
    pub fn new(config: KrakenConfig) -> Self {
        Self {
            config,
            http: Client::new(),
        }
    }

    pub fn execute_intent(
        &self,
        intent: &TradeIntent,
    ) -> Result<ExecutionResult, KrakenProviderError> {
        validate_side(&intent.side)?;
        validate_size_hint(&intent.size_hint)?;

        let nonce = now_unix_ms()?.to_string();
        let pair = normalize_pair(&intent.symbol);
        let mut body = format!(
            "nonce={}&type={}&pair={}&volume={}",
            nonce, intent.side, pair, intent.size_hint
        );

        let ordertype = match intent.order_type.as_str() {
            "market" => "market",
            "limit" => "limit",
            "stop" => "stop-loss",
            "stop_limit" => "stop-loss-limit",
            _ => "market",
        };
        body.push_str(&format!("&ordertype={}", ordertype));

        if ordertype == "limit" {
            if let Some(p) = intent.limit_price {
                body.push_str(&format!("&price={}", p));
            }
        } else if ordertype == "stop-loss" {
            if let Some(p) = intent.stop_price {
                body.push_str(&format!("&price={}", p));
            }
        } else if ordertype == "stop-loss-limit" {
            if let Some(p) = intent.stop_price {
                body.push_str(&format!("&price={}", p));
            }
            if let Some(p) = intent.limit_price {
                body.push_str(&format!("&price2={}", p));
            }
        }

        if let Some(sl) = intent.stop_loss {
            body.push_str("&close[ordertype]=stop-loss");
            body.push_str(&format!("&close[price]={}", sl));
        }

        let tif = intent.time_in_force.to_uppercase();
        match tif.as_str() {
            "GTC" | "IOC" => {
                body.push_str(&format!("&timeinforce={}", tif));
            }
            "DAY" => {
                return Err(KrakenProviderError::InvalidTimeInForce(
                    "DAY time-in-force not supported for Kraken. Use GTC or IOC.".to_string(),
                ));
            }
            _ => {
                // For other values, we can either error or pass through if we support more in future.
                // For safety, error on unknown.
                return Err(KrakenProviderError::InvalidTimeInForce(format!(
                    "Unsupported time-in-force: {}",
                    intent.time_in_force
                )));
            }
        }

        let path = "/0/private/AddOrder";
        let signature = sign_request(&self.config.api_secret, path, &nonce, &body)?;
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);

        let response = self
            .http
            .post(url)
            .header("API-Key", &self.config.api_key)
            .header("API-Sign", signature)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .unwrap_or_else(|_| "unable to decode error body".to_string());
            return Err(KrakenProviderError::UnexpectedHttpStatus(status, body));
        }

        let api_response: KrakenApiResponse = response.json()?;
        if !api_response.error.is_empty() {
            return Err(KrakenProviderError::Api(api_response.error.join(", ")));
        }

        let provider_order_id = api_response
            .result
            .and_then(|result| result.txid.into_iter().next())
            .ok_or(KrakenProviderError::MissingTxid)?;

        let submitted_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| KrakenProviderError::Clock(err.to_string()))?
            .as_millis() as i64;

        Ok(ExecutionResult {
            schema_version: "v0".to_string(),
            intent_id: intent.intent_id.clone(),
            provider: "kraken".to_string(),
            provider_order_id,
            status: "submitted".to_string(),
            submitted_at_unix_ms,
        })
    }
}

fn now_unix_ms() -> Result<i64, KrakenProviderError> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| KrakenProviderError::Clock(err.to_string()))?
        .as_millis() as i64)
}

fn validate_side(side: &str) -> Result<(), KrakenProviderError> {
    if side == "buy" || side == "sell" {
        Ok(())
    } else {
        Err(KrakenProviderError::InvalidSide(side.to_string()))
    }
}

fn validate_size_hint(size_hint: &str) -> Result<(), KrakenProviderError> {
    let volume = size_hint
        .parse::<f64>()
        .map_err(|_| KrakenProviderError::InvalidVolume(size_hint.to_string()))?;
    if volume <= 0.0 {
        return Err(KrakenProviderError::InvalidVolume(size_hint.to_string()));
    }
    Ok(())
}

fn normalize_pair(symbol: &str) -> String {
    symbol.replace('/', "").to_uppercase()
}

fn sign_request(
    api_secret: &str,
    path: &str,
    nonce: &str,
    body: &str,
) -> Result<String, KrakenProviderError> {
    let secret = STANDARD.decode(api_secret)?;

    let mut hasher = Sha256::new();
    hasher.update(nonce.as_bytes());
    hasher.update(body.as_bytes());
    let body_hash = hasher.finalize();

    let mut message = Vec::with_capacity(path.len() + body_hash.len());
    message.extend_from_slice(path.as_bytes());
    message.extend_from_slice(&body_hash);

    let mut mac = Hmac::<Sha512>::new_from_slice(&secret)
        .map_err(|err| KrakenProviderError::Signing(err.to_string()))?;
    mac.update(&message);
    let signature = mac.finalize().into_bytes();

    Ok(STANDARD.encode(signature))
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenApiResponse {
    error: Vec<String>,
    result: Option<KrakenAddOrderResult>,
}

#[derive(Debug, Clone, Deserialize)]
struct KrakenAddOrderResult {
    txid: Vec<String>,
}

#[derive(Debug, Error)]
pub enum KrakenProviderError {
    #[error("missing required environment variable: {0}")]
    MissingEnvVar(&'static str),
    #[error("system clock error: {0}")]
    Clock(String),
    #[error("invalid side: {0}")]
    InvalidSide(String),
    #[error("invalid volume: {0}")]
    InvalidVolume(String),
    #[error("invalid kraken api secret encoding: {0}")]
    SecretDecode(#[from] base64::DecodeError),
    #[error("kraken request signing error: {0}")]
    Signing(String),
    #[error("http transport error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected kraken response status {0}: {1}")]
    UnexpectedHttpStatus(u16, String),
    #[error("kraken api error: {0}")]
    Api(String),
    #[error("kraken response missing txid")]
    MissingTxid,
    #[error("invalid time in force: {0}")]
    InvalidTimeInForce(String),
}
