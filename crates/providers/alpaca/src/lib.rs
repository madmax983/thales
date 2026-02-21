use std::time::{SystemTime, UNIX_EPOCH};

use contracts::{ExecutionResult, TradeIntent};
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
}

impl AlpacaClient {
    pub fn new(config: AlpacaConfig) -> Self {
        Self { config }
    }

    pub fn execute_intent(
        &self,
        intent: &TradeIntent,
    ) -> Result<ExecutionResult, AlpacaProviderError> {
        let submitted_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| AlpacaProviderError::Clock(err.to_string()))?
            .as_millis() as i64;

        Ok(ExecutionResult {
            schema_version: "v0".to_string(),
            intent_id: intent.intent_id.clone(),
            provider: "alpaca".to_string(),
            provider_order_id: format!("alpaca-{}", intent.intent_id),
            status: "submitted".to_string(),
            submitted_at_unix_ms,
        })
    }
}

#[derive(Debug, Error)]
pub enum AlpacaProviderError {
    #[error("missing required environment variable: {0}")]
    MissingEnvVar(&'static str),
    #[error("system clock error: {0}")]
    Clock(String),
}
