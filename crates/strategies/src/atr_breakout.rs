//! ATR Breakout Trading Strategy
//!
//! A volatility-based breakout strategy that uses the Average True Range (ATR)
//! and a Simple Moving Average (SMA) to identify high momentum breakouts.
//!
//! # Entry Conditions
//! - **Long Entry:** Price closes above the SMA + (ATR * multiplier)
//! - **Short Entry:** Price closes below the SMA - (ATR * multiplier)
//!
//! # Exit Conditions
//! - **Long Exit:** Price crosses below the SMA.
//! - **Short Exit:** Price crosses above the SMA.

use crate::indicators::{atr, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration for the ATR Breakout Strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtrBreakoutConfig {
    /// The period for calculating the Average True Range (ATR).
    pub atr_period: usize,
    /// The period for the baseline Simple Moving Average (SMA).
    pub sma_period: usize,
    /// The multiplier for ATR to set the breakout threshold.
    pub breakout_multiplier: f64,
    /// Maximum position size
    pub max_position_size: f64,
    /// The multiplier for ATR to calculate the stop-loss distance from the entry price.
    pub stop_loss_atr_mult: f64,
    /// The trading pair symbol the strategy is applied to.
    pub symbol: String,
}

impl Default for AtrBreakoutConfig {
    fn default() -> Self {
        Self {
            atr_period: 14,
            sma_period: 20,
            breakout_multiplier: 1.5,
            max_position_size: 100.0,
            stop_loss_atr_mult: 2.0,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for AtrBreakoutConfig {}

/// ATR Breakout Strategy
pub struct AtrBreakout {
    config: AtrBreakoutConfig,
}

impl AtrBreakout {
    /// Creates a new instance of the ATR Breakout Strategy.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use strategies::atr_breakout::{AtrBreakout, AtrBreakoutConfig};
    ///
    /// let config = AtrBreakoutConfig {
    ///     atr_period: 14,
    ///     sma_period: 20,
    ///     breakout_multiplier: 1.5,
    ///     max_position_size: 1000.0,
    ///     stop_loss_atr_mult: 2.0,
    ///     symbol: "BTCUSD".to_string(),
    /// };
    ///
    /// let strategy = AtrBreakout::new(config);
    /// ```
    pub fn new(config: AtrBreakoutConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for AtrBreakout {
    fn name(&self) -> &str {
        "AtrBreakout"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if self.config.atr_period == 0 || self.config.sma_period == 0 {
            anyhow::bail!("Periods must be greater than 0");
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        // Calculate SMA
        let sma_series = sma::calculate(data, self.config.sma_period)?;
        let sma_arr = sma_series.f64()?;

        let mut signals = Vec::new();

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let curr_close_opt = close_arr.get(i);
            let prev_close_opt = close_arr.get(i - 1);
            let curr_sma_opt = sma_arr.get(i);
            let prev_sma_opt = sma_arr.get(i - 1);
            let atr_opt = atr_arr.get(i);

            if let (Some(curr_close), Some(prev_close), Some(curr_sma), Some(prev_sma), Some(atr_val)) = (
                curr_close_opt, prev_close_opt, curr_sma_opt, prev_sma_opt, atr_opt,
            ) {
                let upper_band = curr_sma + (atr_val * self.config.breakout_multiplier);
                let lower_band = curr_sma - (atr_val * self.config.breakout_multiplier);

                let prev_upper_band = prev_sma + (atr_val * self.config.breakout_multiplier); // Approximation for crossover
                let prev_lower_band = prev_sma - (atr_val * self.config.breakout_multiplier);

                let long_breakout = prev_close <= prev_upper_band && curr_close > upper_band;
                let short_breakout = prev_close >= prev_lower_band && curr_close < lower_band;

                let long_exit = prev_close >= prev_sma && curr_close < curr_sma;
                let short_exit = prev_close <= prev_sma && curr_close > curr_sma;

                let sl_dist = atr_val * self.config.stop_loss_atr_mult;
                let size_hint = format!("{:.4}", self.config.max_position_size);

                // Long Entry
                if long_breakout {
                    let stop_loss = curr_close - sl_dist;
                    let take_profit = curr_close + (sl_dist * 2.0); // Simple 1:2 RR

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: size_hint.clone(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: Some(take_profit),
                        reason: format!("ATR Breakout Long Entry (Close: {:.2}, Upper: {:.2})", curr_close, upper_band),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Entry
                if short_breakout {
                    let stop_loss = curr_close + sl_dist;
                    let take_profit = curr_close - (sl_dist * 2.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: size_hint.clone(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: Some(take_profit),
                        reason: format!("ATR Breakout Short Entry (Close: {:.2}, Lower: {:.2})", curr_close, lower_band),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit
                if long_exit {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("ATR Breakout Long Exit (Close: {:.2}, SMA: {:.2})", curr_close, curr_sma),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Exit
                if short_exit {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("ATR Breakout Short Exit (Close: {:.2}, SMA: {:.2})", curr_close, curr_sma),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: AtrBreakoutConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_atr_breakout_signals() -> Result<()> {
        let config = AtrBreakoutConfig {
            atr_period: 2,
            sma_period: 2,
            breakout_multiplier: 0.5,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            symbol: "TEST".to_string(),
        };
        let strategy = AtrBreakout::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000],
            "high" =>  &[100.0, 102.0, 110.0, 105.0, 90.0, 100.0],
            "low" =>   &[ 98.0, 100.0, 101.0,  99.0, 80.0,  85.0],
            "close" => &[ 99.0, 101.0, 109.0, 100.0, 82.0,  95.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        assert!(!signals.is_empty(), "Should generate breakout signals given volatile data");

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let config = AtrBreakoutConfig {
            atr_period: 0, // Invalid
            sma_period: 2,
            breakout_multiplier: 1.5,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            symbol: "TEST".to_string(),
        };
        let strategy = AtrBreakout::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64],
            "high" =>  &[100.0],
            "low" =>   &[ 90.0],
            "close" => &[ 95.0]
        )?;

        let result = strategy.generate_signals(&df).await;
        assert!(result.is_err(), "Should fail with invalid periods");

        Ok(())
    }
}
