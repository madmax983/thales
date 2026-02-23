use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    Entry,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    pub signal_type: SignalType,
    pub symbol: String,
    pub side: String,      // "buy" or "sell"
    pub size_hint: String, // e.g., "100" or "0.1" or "max"
    pub confidence: f64,
    pub reason: String,
    pub timestamp_ms: i64,
}

#[async_trait]
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>>;

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()>;
}

pub trait StrategyConfig: Send + Sync {
    // Marker trait for strategy configuration
}
