//! Detrended Price Oscillator (DPO) Breakout Strategy.
//!
//! This strategy uses the Detrended Price Oscillator to identify short-term cycles
//! and trade breakouts when the DPO crosses the zero line, indicating a shift in momentum
//! independent of the long-term trend.

use crate::indicators::{atr, dpo};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Configuration for the [`DpoBreakout`] strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DpoBreakoutConfig {
    pub period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub max_position_size: f64,
    pub symbol: String,
}

impl Default for DpoBreakoutConfig {
    fn default() -> Self {
        Self {
            period: 20,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            max_position_size: 100.0,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for DpoBreakoutConfig {}

pub struct DpoBreakout {
    config: DpoBreakoutConfig,
}

impl DpoBreakout {
    pub fn new(config: DpoBreakoutConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for DpoBreakout {
    fn name(&self) -> &str {
        "DpoBreakout"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Breakout
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?;
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let dpo_series = dpo::calculate(data, self.config.period)?;
        let dpo_arr = dpo_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));

        let size_hint = self.config.max_position_size.to_string();

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);

            let dpo_curr_opt = dpo_arr.get(i).and_then(Decimal::from_f64_retain);
            let dpo_prev_opt = dpo_arr.get(i - 1).and_then(Decimal::from_f64_retain);
            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(dpo_curr), Some(dpo_prev), Some(price)) =
                (dpo_curr_opt, dpo_prev_opt, price_opt)
            {
                // Buy when DPO crosses above 0
                if dpo_curr > Decimal::ZERO && dpo_prev <= Decimal::ZERO {
                    let sl = if let Some(atr_val) = atr_opt {
                        price - (atr_val * atr_mult_dec)
                    } else {
                        price * Decimal::from_f64_retain(0.95).unwrap()
                    };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: size_hint.clone(),
                        confidence: 0.7,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!("DPO ({}) crossed above zero", self.config.period),
                        timestamp_ms: timestamp,
                    });
                }

                // Sell when DPO crosses below 0
                if dpo_curr < Decimal::ZERO && dpo_prev >= Decimal::ZERO {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.7,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("DPO ({}) crossed below zero", self.config.period),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: DpoBreakoutConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_dpo_breakout_signals() -> Result<()> {
        let config = DpoBreakoutConfig {
            period: 4,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        };
        let strategy = DpoBreakout::new(config);

        // Period 4. Shift = 3.
        // We need enough data to calculate DPO and SMA and ATR
        // Let's create a clear cross above 0 then below 0
        // SMA(4)
        let closes = vec![
            10.0, 10.0, 10.0, 10.0, 12.0, 14.0, 16.0, 14.0, 12.0, 10.0, 8.0, 6.0,
        ];
        let highs = vec![
            11.0, 11.0, 11.0, 11.0, 13.0, 15.0, 17.0, 15.0, 13.0, 11.0, 9.0, 7.0,
        ];
        let lows = vec![
            9.0, 9.0, 9.0, 9.0, 11.0, 13.0, 15.0, 13.0, 11.0, 9.0, 7.0, 5.0,
        ];
        let timestamps = vec![
            1000, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000, 11000, 12000,
        ];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(!signals.is_empty());

        let entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry.is_some());

        let exit = signals.iter().find(|s| s.signal_type == SignalType::Exit);
        assert!(exit.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_dpo_breakout_parameter_validation() -> Result<()> {
        let config = DpoBreakoutConfig {
            period: 0, // Invalid, should be caught by indicator
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        };
        let strategy = DpoBreakout::new(config);

        let closes = vec![10.0, 10.0];
        let df = df!(
            "timestamp_unix_ms" => vec![1000, 2000],
            "close" => closes.clone(),
            "high" => closes.clone(),
            "low" => closes.clone()
        )?;

        let result = strategy.generate_signals(&df).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Period must be greater than 0"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_dpo_breakout_edge_cases() -> Result<()> {
        let config = DpoBreakoutConfig {
            period: 4,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        };
        let strategy = DpoBreakout::new(config);

        // Empty dataframe
        let df_empty = DataFrame::default();
        let result = strategy.generate_signals(&df_empty).await;
        assert!(result.is_err());

        // Single row
        let closes = vec![10.0];
        let df_single = df!(
            "timestamp_unix_ms" => vec![1000],
            "close" => closes.clone(),
            "high" => closes.clone(),
            "low" => closes.clone()
        )?;
        let signals = strategy.generate_signals(&df_single).await?;
        assert!(signals.is_empty());

        Ok(())
    }
}
