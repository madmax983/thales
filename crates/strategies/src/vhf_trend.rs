//! Vertical Horizontal Filter (VHF) Trend Following Strategy.
//!
//! This module implements a trend-following strategy using the VHF indicator.
//! The VHF determines whether a market is trending or ranging.

use crate::indicators::{atr, sma, vhf};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VhfTrendFollowingConfig {
    pub vhf_period: usize,
    pub trend_threshold: f64,
    pub sma_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub max_position_size: f64,
    pub symbol: String,
}

impl StrategyConfig for VhfTrendFollowingConfig {}

impl Default for VhfTrendFollowingConfig {
    fn default() -> Self {
        Self {
            vhf_period: 28,
            trend_threshold: 0.4,
            sma_period: 20,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            max_position_size: 100.0,
            symbol: "BTCUSD".to_string(),
        }
    }
}

pub struct VhfTrendFollowing {
    config: VhfTrendFollowingConfig,
}

impl VhfTrendFollowing {
    pub fn new(config: VhfTrendFollowingConfig) -> Self {
        Self { config }
    }

    fn validate_config(config: &VhfTrendFollowingConfig) -> Result<()> {
        if config.vhf_period == 0 {
            anyhow::bail!("vhf_period must be > 0");
        }
        if config.sma_period == 0 {
            anyhow::bail!("sma_period must be > 0");
        }
        if config.atr_period == 0 {
            anyhow::bail!("atr_period must be > 0");
        }
        if config.stop_loss_atr_mult <= 0.0 {
            anyhow::bail!("stop_loss_atr_mult must be > 0");
        }
        if config.max_position_size <= 0.0 {
            anyhow::bail!("max_position_size must be > 0");
        }
        Ok(())
    }
}

#[async_trait]
impl Strategy for VhfTrendFollowing {
    fn name(&self) -> &str {
        "VhfTrendFollowing"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        Self::validate_config(&self.config)?;
        if data.is_empty() {
            return Ok(vec![]);
        }
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate VHF
        let vhf_series = vhf::calculate(data, self.config.vhf_period)?;
        let vhf_arr = vhf_series.f64()?;

        // Calculate SMA
        let sma_series = sma::calculate(data, self.config.sma_period)?;
        let sma_arr = sma_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        let mut active_long = false;
        let mut active_short = false;

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let vhf_curr = vhf_arr.get(i);
            let vhf_prev = vhf_arr.get(i - 1);
            let sma_curr = sma_arr.get(i);
            let sma_prev = sma_arr.get(i - 1);

            let price_curr = close_arr.get(i);
            let price_prev = close_arr.get(i - 1);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(vhf_c),
                Some(vhf_p),
                Some(sma_c),
                Some(sma_p),
                Some(price_c),
                Some(price_p),
                Some(atr_val),
            ) = (vhf_curr, vhf_prev, sma_curr, sma_prev, price_curr, price_prev, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price_c).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Exit Long Logic
                if active_long && (vhf_c < self.config.trend_threshold || price_c < sma_c) {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Long Exit: Trend weakening or Price below SMA".to_string(),
                        timestamp_ms: timestamp,
                    });
                    active_long = false;
                }

                // Exit Short Logic
                if active_short && (vhf_c < self.config.trend_threshold || price_c > sma_c) {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Short Exit: Trend weakening or Price above SMA".to_string(),
                        timestamp_ms: timestamp,
                    });
                    active_short = false;
                }

                // Long Entry
                if vhf_c > self.config.trend_threshold && price_c > sma_c && (vhf_p <= self.config.trend_threshold || price_p <= sma_p) && !active_long {
                    let sl = price_dec - (atr_dec * sl_mult);
                    let risk = price_dec - sl;
                    let tp = price_dec + (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!("VHF Trend Up: VHF {:.2} > {}, Price {:.2} > SMA {:.2}", vhf_c, self.config.trend_threshold, price_c, sma_c),
                        timestamp_ms: timestamp,
                    });
                    active_long = true;
                }
                // Short Entry
                else if vhf_c > self.config.trend_threshold && price_c < sma_c && (vhf_p <= self.config.trend_threshold || price_p >= sma_p) && !active_short {
                    let sl = price_dec + (atr_dec * sl_mult);
                    let risk = sl - price_dec;
                    let tp = price_dec - (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Entry Short
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!("VHF Trend Down: VHF {:.2} > {}, Price {:.2} < SMA {:.2}", vhf_c, self.config.trend_threshold, price_c, sma_c),
                        timestamp_ms: timestamp,
                    });
                    active_short = true;
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: VhfTrendFollowingConfig = serde_json::from_value(params)?;
        Self::validate_config(&new_config)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_parameter_validation() {
        let mut config = VhfTrendFollowingConfig::default();
        config.vhf_period = 0;
        let res = VhfTrendFollowing::validate_config(&config);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "vhf_period must be > 0");

        config.vhf_period = 10;
        config.sma_period = 0;
        let res = VhfTrendFollowing::validate_config(&config);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "sma_period must be > 0");

        config.sma_period = 10;
        config.atr_period = 0;
        let res = VhfTrendFollowing::validate_config(&config);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "atr_period must be > 0");

        config.atr_period = 10;
        config.stop_loss_atr_mult = 0.0;
        let res = VhfTrendFollowing::validate_config(&config);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "stop_loss_atr_mult must be > 0");

        config.stop_loss_atr_mult = 2.0;
        config.max_position_size = 0.0;
        let res = VhfTrendFollowing::validate_config(&config);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "max_position_size must be > 0");
    }

    #[tokio::test]
    async fn test_edge_cases() -> Result<()> {
        let config = VhfTrendFollowingConfig {
            vhf_period: 2,
            trend_threshold: 0.4,
            sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        };
        let strategy = VhfTrendFollowing::new(config);

        // Empty data
        let df_empty = DataFrame::default();
        let signals = strategy.generate_signals(&df_empty).await?;
        assert!(signals.is_empty());

        // Single data point
        let df_single = df!(
            "timestamp_unix_ms" => &[1000i64],
            "close" => &[10.0],
            "high"  => &[10.5],
            "low"   => &[9.5],
            "volume"=> &[10.0]
        )?;
        let signals = strategy.generate_signals(&df_single).await?;
        assert!(signals.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_entry_and_exit_signal_generation() -> Result<()> {
        let config = VhfTrendFollowingConfig {
            vhf_period: 2,
            trend_threshold: 0.5,
            sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        };
        let strategy = VhfTrendFollowing::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000],
            "close" => &[10.0, 10.0, 10.0, 10.0, 15.0, 16.0, 9.0, 8.0],
            "high"  => &[10.0, 10.0, 10.0, 10.0, 15.0, 16.0, 9.0, 8.0],
            "low"   => &[10.0, 10.0, 10.0, 10.0, 15.0, 16.0, 9.0, 8.0],
            "volume"=> &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(!signals.is_empty());
        Ok(())
    }
}
