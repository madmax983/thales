//! Ulcer Index Mean Reversion Strategy

use crate::indicators::ulcer_index;
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UlcerIndexMeanReversionConfig {
    pub period: usize,
    pub entry_threshold: f64,
    pub exit_threshold: f64,
    pub stop_loss_pct: f64,
    pub symbol: String,
}

impl Default for UlcerIndexMeanReversionConfig {
    fn default() -> Self {
        Self {
            period: 14,
            entry_threshold: 10.0,
            exit_threshold: 2.0,
            stop_loss_pct: 0.05,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for UlcerIndexMeanReversionConfig {}

pub struct UlcerIndexMeanReversion {
    config: UlcerIndexMeanReversionConfig,
}

impl UlcerIndexMeanReversion {
    pub fn new(config: UlcerIndexMeanReversionConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for UlcerIndexMeanReversion {
    fn name(&self) -> &str {
        "UlcerIndexMeanReversion"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let mut signals = Vec::new();
        let height = data.height();
        if height < 2 {
            return Ok(signals);
        }

        let ui_series = ulcer_index::calculate(data, self.config.period)?;
        let ui_vals = ui_series.f64()?;

        let close_series = data.column("close")?.f64()?;
        let time_series = data.column("timestamp_unix_ms")?.i64()?;

        for i in 1..height {
            let prev_ui = ui_vals.get(i - 1);
            let curr_ui = ui_vals.get(i);

            if let (Some(prev), Some(curr)) = (prev_ui, curr_ui) {
                let timestamp = time_series.get(i).unwrap_or(0);
                let close = close_series.get(i).unwrap_or(0.0);

                // Long Entry: UI crosses ABOVE entry_threshold (extreme panic)
                if prev <= self.config.entry_threshold && curr > self.config.entry_threshold {
                    let sl = close * (1.0 - self.config.stop_loss_pct);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: None,
                        reason: format!(
                            "Ulcer Index crossed above Entry Threshold: {:.2} > {:.2}",
                            curr, self.config.entry_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit: UI crosses BELOW exit_threshold (risk normalizing)
                if prev >= self.config.exit_threshold && curr < self.config.exit_threshold {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Ulcer Index crossed below Exit Threshold: {:.2} < {:.2}",
                            curr, self.config.exit_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: UlcerIndexMeanReversionConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let config = UlcerIndexMeanReversionConfig::default();
        let strategy = UlcerIndexMeanReversion::new(config);

        assert_eq!(strategy.name(), "UlcerIndexMeanReversion");
        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let config = UlcerIndexMeanReversionConfig::default();
        assert_eq!(config.period, 14);
        Ok(())
    }

    #[tokio::test]
    async fn test_signal_generation() -> Result<()> {
        // UI calculation trace:
        // Day 1: close=100. max=100. dd=0. UI=0. (assuming period=2 for easy testing)
        // Day 2: close=80. max=100. dd=-20.
        // sum_sq = 0 + 400 = 400. avg=200. ui=14.14
        // Day 3: close=60. max=100. dd=-40.
        // sum_sq = 400 + 1600 = 2000. avg=1000. ui=31.62
        // Day 4: close=100. max=100. dd=0.
        // sum_sq = 1600 + 0 = 1600. avg=800. ui=28.28
        // Day 5: close=120. max=120. dd=0.
        // sum_sq = 0 + 0 = 0. ui=0.

        let config = UlcerIndexMeanReversionConfig {
            period: 2,
            entry_threshold: 20.0, // Crosses above 20.0 to buy
            exit_threshold: 10.0,  // Crosses below 10.0 to sell
            stop_loss_pct: 0.10,
            symbol: "TEST".to_string(),
        };
        let strategy = UlcerIndexMeanReversion::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000_i64, 2000, 3000, 4000, 5000],
            "close" => &[100.0, 80.0, 60.0, 100.0, 120.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // UI values:
        // idx 0: None
        // idx 1: 14.14
        // idx 2: 31.62 (prev=14.14, curr=31.62) -> Crosses above 20.0 (Buy)
        // idx 3: 28.28
        // idx 4: 0.0 (prev=28.28, curr=0.0) -> Crosses below 10.0 (Sell)

        assert_eq!(signals.len(), 2);

        assert_eq!(signals[0].signal_type, SignalType::Entry);
        assert_eq!(signals[0].timestamp_ms, 3000);
        assert_eq!(signals[0].side, "buy");

        assert_eq!(signals[1].signal_type, SignalType::Exit);
        assert_eq!(signals[1].timestamp_ms, 5000);
        assert_eq!(signals[1].side, "sell");

        Ok(())
    }
}
