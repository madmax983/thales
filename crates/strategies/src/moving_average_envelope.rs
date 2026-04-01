//! Moving Average Envelopes Trading Strategy
//!
//! A mean-reversion and momentum strategy based on Moving Average Envelopes (MAE).
//!
//! # Entry Conditions
//! - **Long Entry:** Price crosses below the lower band (oversold).
//! - **Short Entry:** Price crosses above the upper band (overbought).
//!
//! # Exit Conditions
//! - **Long Exit:** Price crosses above the SMA.
//! - **Short Exit:** Price crosses below the SMA.

use crate::indicators::{atr, mae};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration for the Moving Average Envelopes Strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovingAverageEnvelopeConfig {
    /// Period for the SMA.
    pub period: usize,
    /// Percentage distance for the envelopes (e.g., 2.5 for 2.5%).
    pub percentage: f64,
    /// Maximum position size.
    pub max_position_size: f64,
    /// Multiplier for ATR to calculate the stop-loss distance.
    pub stop_loss_atr_mult: f64,
    /// Period for calculating the Average True Range (ATR).
    pub atr_period: usize,
    /// The trading pair symbol.
    pub symbol: String,
}

impl Default for MovingAverageEnvelopeConfig {
    fn default() -> Self {
        Self {
            period: 20,
            percentage: 2.5,
            max_position_size: 100.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "BTCUSD".to_string(),
        }
    }
}

impl StrategyConfig for MovingAverageEnvelopeConfig {}

/// Moving Average Envelopes Strategy
pub struct MovingAverageEnvelope {
    config: MovingAverageEnvelopeConfig,
}

impl MovingAverageEnvelope {
    /// Creates a new instance of the strategy.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use strategies::moving_average_envelope::{MovingAverageEnvelope, MovingAverageEnvelopeConfig};
    ///
    /// let config = MovingAverageEnvelopeConfig::default();
    /// let strategy = MovingAverageEnvelope::new(config);
    /// ```
    pub fn new(config: MovingAverageEnvelopeConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for MovingAverageEnvelope {
    fn name(&self) -> &str {
        "MovingAverageEnvelope"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if self.config.period == 0 {
            anyhow::bail!("Period must be greater than 0");
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate MAE
        let (upper_series, lower_series, sma_series) = mae::calculate(
            data,
            self.config.period,
            self.config.percentage,
        )?;

        let upper_arr: &Float64Chunked = upper_series.f64()?;
        let lower_arr: &Float64Chunked = lower_series.f64()?;
        let sma_arr: &Float64Chunked = sma_series.f64()?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let price_curr = close_arr.get(i);
            let price_prev = close_arr.get(i - 1);

            let upper_curr = upper_arr.get(i);
            let lower_curr = lower_arr.get(i);
            let sma_curr = sma_arr.get(i);

            let upper_prev = upper_arr.get(i - 1);
            let lower_prev = lower_arr.get(i - 1);
            let sma_prev = sma_arr.get(i - 1);

            let atr_opt = atr_arr.get(i);

            if let (
                Some(p), Some(pp),
                Some(u), Some(pu),
                Some(l), Some(pl),
                Some(s), Some(ps)
            ) = (
                price_curr, price_prev,
                upper_curr, upper_prev,
                lower_curr, lower_prev,
                sma_curr, sma_prev
            ) {
                let cross_below_lower = pp >= pl && p < l;
                let cross_above_upper = pp <= pu && p > u;

                let cross_above_sma = pp <= ps && p > s;
                let cross_below_sma = pp >= ps && p < s;

                // Stop loss calculation
                let sl_dist = if let Some(atr_val) = atr_opt {
                    atr_val * self.config.stop_loss_atr_mult
                } else {
                    p * 0.05 // Fallback 5%
                };

                let size_hint = format!("{:.4}", self.config.max_position_size);

                // Long Entry
                if cross_below_lower {
                    let stop_loss = p - sl_dist;
                    let take_profit = p + (sl_dist * 2.0); // Simple 1:2 RR

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: size_hint.clone(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: Some(take_profit),
                        reason: format!("MAE Long Entry (Price: {:.2}, Lower: {:.2})", p, l),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Entry
                if cross_above_upper {
                    let stop_loss = p + sl_dist;
                    let take_profit = p - (sl_dist * 2.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: size_hint.clone(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: Some(take_profit),
                        reason: format!("MAE Short Entry (Price: {:.2}, Upper: {:.2})", p, u),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit
                if cross_above_sma {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("MAE Long Exit (Price: {:.2}, SMA: {:.2})", p, s),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Exit
                if cross_below_sma {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("MAE Short Exit (Price: {:.2}, SMA: {:.2})", p, s),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: MovingAverageEnvelopeConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_mae_strategy_signals() -> Result<()> {
        let config = MovingAverageEnvelopeConfig {
            period: 2,
            percentage: 5.0,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = MovingAverageEnvelope::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "high" =>  &[100.0, 105.0, 110.0, 90.0, 80.0],
            "low" =>   &[ 90.0,  95.0,  95.0, 80.0, 70.0],
            "close" => &[100.0, 100.0, 106.0, 90.0, 95.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // This is mainly a test that logic runs and generates exits/entries based on the data provided
        assert!(
            !signals.is_empty(),
            "Should generate some signals given typical data behavior"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let config = MovingAverageEnvelopeConfig {
            period: 0, // Invalid
            percentage: 5.0,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = MovingAverageEnvelope::new(config);

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
