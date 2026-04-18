//! Weighted Close Trend Strategy
//!
//! # Core Concept
//! The strategy generates signals based on the crossover between the Weighted Close and its SMA:
//! - **Bullish Trend:** Weighted Close crosses above its SMA.
//! - **Bearish Trend:** Weighted Close crosses below its SMA.
//!
//! Risk management is handled by an ATR-based stop-loss and a 2:1 risk-reward take-profit.

use crate::indicators::{atr, weighted_close};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedCloseTrendConfig {
    pub sma_period: usize,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}

impl Default for WeightedCloseTrendConfig {
    fn default() -> Self {
        Self {
            sma_period: 20,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for WeightedCloseTrendConfig {}

pub struct WeightedCloseTrend {
    config: WeightedCloseTrendConfig,
}

impl WeightedCloseTrend {
    pub fn new(config: WeightedCloseTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for WeightedCloseTrend {
    fn name(&self) -> &str {
        "WeightedCloseTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            bail!("Data cannot be empty");
        }

        let close_col = data.column("close")?.cast(&DataType::Float64)?;
        let _close_f64 = close_col.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Close price as String to parse to Decimal
        let close_series = data.column("close")?.cast(&DataType::String)?;
        let close_ca = close_series.str()?;

        // Calculate Weighted Close Indicator (Returns String Series)
        let wc_series = weighted_close::calculate(data)?;
        let wc_ca = wc_series.str()?;

        // ATR for Stop Loss/Take profit
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        // Compute SMA of Weighted Close iteratively using Decimal
        let mut wc_sma_arr = vec![None; wc_ca.len()];
        let mut window = std::collections::VecDeque::new();
        let mut sum = Decimal::ZERO;
        let period_dec = Decimal::from_usize(self.config.sma_period)
            .context("Invalid period for Decimal conversion")?;

        for (i, wc_opt) in wc_ca.into_iter().enumerate() {
            if let Some(wc_str) = wc_opt {
                if wc_str == "NaN" {
                    window.clear();
                    sum = Decimal::ZERO;
                    continue;
                }

                let wc_dec = Decimal::from_str(wc_str).map_err(|e| anyhow::anyhow!(e))?;
                sum += wc_dec;
                window.push_back(wc_dec);

                if window.len() > self.config.sma_period {
                    if let Some(old) = window.pop_front() {
                        sum -= old;
                    }
                }

                if window.len() == self.config.sma_period {
                    wc_sma_arr[i] = Some(sum / period_dec);
                }
            } else {
                window.clear();
                sum = Decimal::ZERO;
            }
        }

        for i in 1..close_ca.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let wc_curr_str = wc_ca.get(i);
            let wc_prev_str = wc_ca.get(i - 1);

            let sma_curr = wc_sma_arr[i];
            let sma_prev = wc_sma_arr[i - 1];

            let price_str = close_ca.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(wc_c_s),
                Some(wc_p_s),
                Some(sma_c),
                Some(sma_p),
                Some(p_s),
                Some(atr_val),
            ) = (
                wc_curr_str,
                wc_prev_str,
                sma_curr,
                sma_prev,
                price_str,
                atr_opt,
            ) {
                if wc_c_s == "NaN" || wc_p_s == "NaN" || p_s == "NaN" {
                    continue;
                }

                let wc_c = Decimal::from_str(wc_c_s)?;
                let wc_p = Decimal::from_str(wc_p_s)?;
                let price_dec = Decimal::from_str(p_s)?;
                let atr_dec = Decimal::from_f64_retain(atr_val).context("Invalid ATR")?;

                // Long Entry
                if wc_p <= sma_p && wc_c > sma_c {
                    let sl = price_dec - (atr_dec * sl_mult);
                    let risk = price_dec - sl;
                    let tp = price_dec + (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: "Weighted Close crossed above SMA (Bullish Trend)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Entry
                if wc_p >= sma_p && wc_c < sma_c {
                    let sl = price_dec + (atr_dec * sl_mult);
                    let risk = sl - price_dec;
                    let tp = price_dec - (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: "Weighted Close crossed below SMA (Bearish Trend)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit
                if wc_p >= sma_p && wc_c < sma_c {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Weighted Close crossed below SMA (Long Exit)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Exit
                if wc_p <= sma_p && wc_c > sma_c {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Weighted Close crossed above SMA (Short Exit)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: WeightedCloseTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let config = WeightedCloseTrendConfig::default();
        let strategy = WeightedCloseTrend::new(config);
        let df = DataFrame::default();
        let res = strategy.generate_signals(&df).await;
        assert!(res.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = WeightedCloseTrend::new(WeightedCloseTrendConfig::default());
        let new_params = serde_json::json!({
            "sma_period": 100,
            "atr_period": 10,
            "stop_loss_atr_mult": 1.5,
            "symbol": "BTCUSD"
        });
        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.sma_period, 100);
        assert_eq!(strategy.config.symbol, "BTCUSD");
        Ok(())
    }

    #[tokio::test]
    async fn test_signal_generation() -> Result<()> {
        let config = WeightedCloseTrendConfig {
            sma_period: 2,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            symbol: "AAPL".to_string(),
        };
        let strategy = WeightedCloseTrend::new(config);

        let times: Vec<i64> = vec![1000, 2000, 3000, 4000, 5000, 6000, 7000];
        let closes: Vec<f64> = vec![100.0, 105.0, 102.0, 104.0, 101.0, 103.0, 100.0];
        let highs: Vec<f64> = vec![105.0, 110.0, 105.0, 105.0, 105.0, 105.0, 105.0];
        let lows: Vec<f64> = vec![95.0, 95.0, 95.0, 100.0, 100.0, 100.0, 100.0];

        let mut df = df!(
            "timestamp_unix_ms" => times,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        df.try_apply("close", |s| s.cast(&DataType::Float64))?;

        let signals = strategy.generate_signals(&df).await?;

        let _entry_signals = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect::<Vec<_>>();
        let _exit_signals = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect::<Vec<_>>();

                assert!(!_entry_signals.is_empty(), "Should generate entry signals");
        assert!(!_exit_signals.is_empty(), "Should generate exit signals");
        for signal in &_entry_signals {
            assert!(signal.stop_loss.is_some(), "Entry signals must have a stop loss");
        }
        Ok(())
    }
}
