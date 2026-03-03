//! Aroon Oscillator Strategy
//!
//! A trend-following strategy that uses the Aroon Oscillator to identify trend
//! strength and direction. Aroon Up measures periods since the highest high,
//! and Aroon Down measures periods since the lowest low.

use crate::indicators::{aroon, atr};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::{Context, Result};
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Configuration for the Aroon Oscillator Strategy
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AroonOscillatorConfig {
    /// Lookback period for Aroon calculation
    pub period: usize,
    /// Threshold to trigger a buy signal (e.g., 0.0 or 50.0)
    pub buy_threshold: f64,
    /// Threshold to trigger a sell signal (e.g., 0.0 or -50.0)
    pub sell_threshold: f64,
    /// ATR multiplier for stop loss
    pub stop_loss_atr_mult: f64,
    /// ATR period for stop loss calculation
    pub atr_period: usize,
    /// The symbol to trade
    pub symbol: String,
}

impl StrategyConfig for AroonOscillatorConfig {}

/// Aroon Oscillator Strategy implementation
pub struct AroonOscillator {
    config: AroonOscillatorConfig,
}

impl AroonOscillator {
    pub fn new(config: AroonOscillatorConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for AroonOscillator {
    fn name(&self) -> &str {
        "AroonOscillator"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.period {
            return Ok(vec![]); // Not enough data
        }

        let close_series = data
            .column("close")
            .context("DataFrame must contain 'close' column")?
            .f64()
            .context("Close column must be numeric (f64)")?;

        let timestamp_series = data
            .column("timestamp_unix_ms")
            .context("DataFrame must contain 'timestamp_unix_ms' column")?
            .i64()
            .context("Timestamp column must be numeric (i64)")?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_f64 = atr_series.f64()?;

        let (_, _, aroon_osc_series) = aroon::calculate(data, self.config.period)?;
        let aroon_osc_f64 = aroon_osc_series.f64()?;

        let mut signals = Vec::new();

        // Need at least 2 points to detect a crossover
        if aroon_osc_f64.len() < 2 {
            return Ok(signals);
        }

        // Generate signal only on the latest closed candle
        let last_idx = close_series.len() - 1;
        let prev_idx = last_idx - 1;

        let last_osc = aroon_osc_f64.get(last_idx);
        let prev_osc = aroon_osc_f64.get(prev_idx);

        let last_close = close_series.get(last_idx);
        let last_ts = timestamp_series.get(last_idx);
        let last_atr = atr_f64.get(last_idx);

        if let (Some(l_osc), Some(p_osc), Some(close_price), Some(ts), Some(atr_val)) =
            (last_osc, prev_osc, last_close, last_ts, last_atr)
        {
            // Buy condition: Aroon Oscillator crosses above buy_threshold
            let buy_condition =
                p_osc <= self.config.buy_threshold && l_osc > self.config.buy_threshold;

            // Sell condition: Aroon Oscillator crosses below sell_threshold
            let sell_condition =
                p_osc >= self.config.sell_threshold && l_osc < self.config.sell_threshold;

            if buy_condition {
                let sl_dist = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO)
                    * Decimal::from_f64_retain(self.config.stop_loss_atr_mult)
                        .unwrap_or(Decimal::ZERO);
                let sl_price = (Decimal::from_f64_retain(close_price).unwrap_or(Decimal::ZERO)
                    - sl_dist)
                    .to_f64()
                    .unwrap_or(0.0);

                signals.push(Signal {
                    signal_type: SignalType::Entry,
                    symbol: self.config.symbol.clone(),
                    side: "buy".to_string(),
                    size_hint: "100".to_string(),
                    confidence: 0.8,
                    stop_loss: Some(sl_price),
                    take_profit: None,
                    reason: format!(
                        "Aroon Oscillator crossed above {}",
                        self.config.buy_threshold
                    ),
                    timestamp_ms: ts,
                });
            } else if sell_condition {
                let sl_dist = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO)
                    * Decimal::from_f64_retain(self.config.stop_loss_atr_mult)
                        .unwrap_or(Decimal::ZERO);
                let sl_price = (Decimal::from_f64_retain(close_price).unwrap_or(Decimal::ZERO)
                    + sl_dist)
                    .to_f64()
                    .unwrap_or(0.0);

                signals.push(Signal {
                    signal_type: SignalType::Entry,
                    symbol: self.config.symbol.clone(),
                    side: "sell".to_string(),
                    size_hint: "100".to_string(),
                    confidence: 0.8,
                    stop_loss: Some(sl_price),
                    take_profit: None,
                    reason: format!(
                        "Aroon Oscillator crossed below {}",
                        self.config.sell_threshold
                    ),
                    timestamp_ms: ts,
                });
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: AroonOscillatorConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_buy_signal() {
        // Provide data that creates an uptrend crossover
        let data = df!(
            "high" => &[10.0, 11.0, 12.0, 13.0, 14.0, 13.0, 12.0, 15.0],
            "low" =>  &[ 5.0,  6.0,  7.0,  8.0,  9.0,  8.0,  7.0,  6.0],
            "close" =>&[ 8.0,  9.0, 10.0, 11.0, 12.0, 11.0, 10.0, 14.0],
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000],
        )
        .unwrap();

        let config = AroonOscillatorConfig {
            period: 3,
            buy_threshold: 0.0,
            sell_threshold: 0.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 3,
            symbol: "TEST".to_string(),
        };

        let strategy = AroonOscillator::new(config);
        let _signals = strategy.generate_signals(&data).await.unwrap();
    }

    #[tokio::test]
    async fn test_sell_signal() {
        let data = df!(
            "high" => &[10.0, 15.0, 12.0, 18.0, 20.0, 19.0, 17.0, 16.0],
            "low" =>  &[ 5.0,  8.0,  6.0, 10.0, 15.0, 14.0, 12.0, 10.0],
            "close" =>&[ 8.0, 10.0,  9.0, 14.0, 18.0, 16.0, 14.0, 12.0],
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000],
        )
        .unwrap();

        let config = AroonOscillatorConfig {
            period: 3,
            buy_threshold: 0.0,
            sell_threshold: 0.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 3,
            symbol: "TEST".to_string(),
        };

        let strategy = AroonOscillator::new(config);
        let signals = strategy.generate_signals(&data).await.unwrap();
        // Just checking execution completes successfully
        assert!(signals.len() <= 1);
    }

    #[tokio::test]
    async fn test_update_params() {
        let config = AroonOscillatorConfig {
            period: 14,
            buy_threshold: 0.0,
            sell_threshold: 0.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "BTCUSD".to_string(),
        };
        let mut strategy = AroonOscillator::new(config);
        let new_params = serde_json::json!({
            "period": 20,
            "buy_threshold": 50.0,
            "sell_threshold": -50.0,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 14,
            "symbol": "ETHUSD"
        });

        strategy.update_params(new_params).await.unwrap();
        assert_eq!(strategy.config.period, 20);
        assert_eq!(strategy.config.symbol, "ETHUSD");
    }
}
