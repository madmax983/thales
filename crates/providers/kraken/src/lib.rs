use std::time::{SystemTime, UNIX_EPOCH};

use contracts::{ExecutionResult, TradeIntent};
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
}

impl KrakenClient {
    pub fn new(config: KrakenConfig) -> Self {
        Self { config }
    }

    pub fn execute_intent(
        &self,
        intent: &TradeIntent,
    ) -> Result<ExecutionResult, KrakenProviderError> {
        let submitted_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| KrakenProviderError::Clock(err.to_string()))?
            .as_millis() as i64;

        Ok(ExecutionResult {
            schema_version: "v0".to_string(),
            intent_id: intent.intent_id.clone(),
            provider: "kraken".to_string(),
            provider_order_id: format!("kraken-{}", intent.intent_id),
            status: "submitted".to_string(),
            submitted_at_unix_ms,
        })
    }
}

#[derive(Debug, Error)]
pub enum KrakenProviderError {
    #[error("missing required environment variable: {0}")]
    MissingEnvVar(&'static str),
    #[error("system clock error: {0}")]
    Clock(String),
}
