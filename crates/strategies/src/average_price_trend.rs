//! Average Price Trend Strategy
//!
//! A trend-following strategy based on the Average Price (OHLC4) and its Simple Moving Average (SMA).
//!
//! # Core Concept
//! The strategy generates signals based on the crossover between the Average Price and its SMA:
//! - **Bullish Trend:** Average Price crosses above its SMA.
//! - **Bearish Trend:** Average Price crosses below its SMA.
//!
//! Risk management is handled by an ATR-based stop-loss and a 2:1 risk-reward take-profit.

use crate::indicators::{atr, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::{bail, Result};
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AveragePriceTrendConfig {
    pub sma_period: usize,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub max_position_size: f64,
    pub symbol: String,
}

impl Default for AveragePriceTrendConfig {
    fn default() -> Self {
        Self {
            sma_period: 20,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            max_position_size: 100.0,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for AveragePriceTrendConfig {}

pub struct AveragePriceTrend {
    config: AveragePriceTrendConfig,
}

impl AveragePriceTrend {
    pub fn new(config: AveragePriceTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for AveragePriceTrend {
    fn name(&self) -> &str {
        "AveragePriceTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            bail!("Data cannot be empty");
        }
        if self.config.max_position_size <= 0.0 {
            bail!("max_position_size must be greater than zero");
        }
        if self.config.stop_loss_atr_mult <= 0.0 {
            bail!("stop_loss_atr_mult must be greater than zero");
        }

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let close_series = data.column("close")?.cast(&DataType::Float64)?;
        let close_arr = close_series.f64()?;

        let open_series = data.column("open")?.cast(&DataType::Float64)?;
        let open_arr = open_series.f64()?;

        let high_series = data.column("high")?.cast(&DataType::Float64)?;
        let high_arr = high_series.f64()?;

        let low_series = data.column("low")?.cast(&DataType::Float64)?;
        let low_arr = low_series.f64()?;

        let mut avg_prices = Vec::with_capacity(data.height());
        for i in 0..data.height() {
            let o = open_arr.get(i).unwrap_or(0.0);
            let h = high_arr.get(i).unwrap_or(0.0);
            let l = low_arr.get(i).unwrap_or(0.0);
            let c = close_arr.get(i).unwrap_or(0.0);
            avg_prices.push((o + h + l + c) / 4.0);
        }

        let ap_series = Series::new("close", avg_prices);
        let ap_df = DataFrame::new(vec![ap_series.clone()])?;

        let sma_series = sma::calculate(&ap_df, self.config.sma_period)?;
        let sma_arr = sma_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let ap_arr = ap_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        let size_hint = format!("{:.4}", self.config.max_position_size);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let ap_curr = ap_arr.get(i);
            let ap_prev = ap_arr.get(i - 1);

            let sma_curr = sma_arr.get(i);
            let sma_prev = sma_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (Some(ap_c), Some(ap_p), Some(sma_c), Some(sma_p), Some(price), Some(atr_val)) =
                (ap_curr, ap_prev, sma_curr, sma_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Long Entry
                if ap_p <= sma_p && ap_c > sma_c {
                    let sl = price_dec - (atr_dec * sl_mult);
                    let risk = price_dec - sl;
                    let tp = price_dec + (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: size_hint.clone(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: "Average Price crossed above SMA (Bullish Trend)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Entry
                if ap_p >= sma_p && ap_c < sma_c {
                    let sl = price_dec + (atr_dec * sl_mult);
                    let risk = sl - price_dec;
                    let tp = price_dec - (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: size_hint.clone(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: "Average Price crossed below SMA (Bearish Trend)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit
                if ap_p >= sma_p && ap_c < sma_c {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Average Price crossed below SMA (Long Exit)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Exit
                if ap_p <= sma_p && ap_c > sma_c {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Average Price crossed above SMA (Short Exit)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: AveragePriceTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let config = AveragePriceTrendConfig::default();
        let strategy = AveragePriceTrend::new(config);
        let df = DataFrame::default();
        let res = strategy.generate_signals(&df).await;
        assert!(res.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = AveragePriceTrend::new(AveragePriceTrendConfig::default());
        let new_params = serde_json::json!({
            "sma_period": 100,
            "atr_period": 10,
            "stop_loss_atr_mult": 1.5,
            "max_position_size": 50.0,
            "symbol": "BTCUSD"
        });
        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.sma_period, 100);
        assert_eq!(strategy.config.symbol, "BTCUSD");
        assert_eq!(strategy.config.max_position_size, 50.0);
        Ok(())
    }

    #[tokio::test]
    async fn test_validation_bounds() -> Result<()> {
        let config = AveragePriceTrendConfig {
            max_position_size: 0.0,
            ..Default::default()
        };
        let strategy = AveragePriceTrend::new(config);

        let times: Vec<i64> = vec![1000];
        let opens: Vec<f64> = vec![95.0];
        let closes: Vec<f64> = vec![100.0];
        let highs: Vec<f64> = vec![105.0];
        let lows: Vec<f64> = vec![95.0];

        let mut df = df!(
            "timestamp_unix_ms" => times,
            "open" => opens,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        df.try_apply("open", |s| s.cast(&DataType::Float64))?;
        df.try_apply("close", |s| s.cast(&DataType::Float64))?;
        df.try_apply("high", |s| s.cast(&DataType::Float64))?;
        df.try_apply("low", |s| s.cast(&DataType::Float64))?;

        let res = strategy.generate_signals(&df).await;
        assert!(res.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_signal_generation() -> Result<()> {
        let config = AveragePriceTrendConfig {
            sma_period: 2,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            max_position_size: 100.0,
            symbol: "AAPL".to_string(),
        };
        let strategy = AveragePriceTrend::new(config);

        let times: Vec<i64> = vec![1000, 2000, 3000, 4000, 5000, 6000, 7000];
        let opens: Vec<f64> = vec![95.0, 100.0, 98.0, 101.0, 99.0, 102.0, 98.0];
        let closes: Vec<f64> = vec![100.0, 105.0, 102.0, 104.0, 101.0, 103.0, 100.0];
        let highs: Vec<f64> = vec![105.0, 110.0, 105.0, 105.0, 105.0, 105.0, 105.0];
        let lows: Vec<f64> = vec![95.0, 95.0, 95.0, 100.0, 100.0, 100.0, 100.0];

        let mut df = df!(
            "timestamp_unix_ms" => times,
            "open" => opens,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        df.try_apply("open", |s| s.cast(&DataType::Float64))?;
        df.try_apply("close", |s| s.cast(&DataType::Float64))?;
        df.try_apply("high", |s| s.cast(&DataType::Float64))?;
        df.try_apply("low", |s| s.cast(&DataType::Float64))?;

        let signals = strategy.generate_signals(&df).await?;

        let entry_signals = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect::<Vec<_>>();
        let exit_signals = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect::<Vec<_>>();

        assert!(
            !entry_signals.is_empty(),
            "Should generate entry signals on crossover"
        );
        assert!(
            !exit_signals.is_empty(),
            "Should generate exit signals on crossover"
        );

        Ok(())
    }
}
