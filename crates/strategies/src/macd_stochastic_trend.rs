//! MACD and Stochastic Trend Following Strategy
//!
//! A dual-indicator strategy combining the MACD for trend direction
//! and the Stochastic Oscillator for momentum and entry timing.
//!
//! # Entry Conditions
//! - **Long Entry:** MACD line > Signal line AND Stochastic %K crosses above %D in the oversold region (< 20).
//! - **Short Entry:** MACD line < Signal line AND Stochastic %K crosses below %D in the overbought region (> 80).
//!
//! # Exit Conditions
//! - **Long Exit:** MACD line crosses below Signal line OR Stochastic %K crosses below %D in overbought region.
//! - **Short Exit:** MACD line crosses above Signal line OR Stochastic %K crosses above %D in oversold region.

use crate::indicators::{atr, macd, stochastic};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration for the MacdStochasticTrend strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacdStochasticTrendConfig {
    pub macd_fast_period: usize,
    pub macd_slow_period: usize,
    pub macd_signal_period: usize,
    pub stoch_k_period: usize,
    pub stoch_d_period: usize,
    pub stoch_oversold: f64,
    pub stoch_overbought: f64,
    pub max_position_size: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for MacdStochasticTrendConfig {
    fn default() -> Self {
        Self {
            macd_fast_period: 12,
            macd_slow_period: 26,
            macd_signal_period: 9,
            stoch_k_period: 14,
            stoch_d_period: 3,
            stoch_oversold: 20.0,
            stoch_overbought: 80.0,
            max_position_size: 100.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "BTCUSD".to_string(),
        }
    }
}

impl StrategyConfig for MacdStochasticTrendConfig {}

/// MACD Stochastic Trend Following Strategy
pub struct MacdStochasticTrend {
    config: MacdStochasticTrendConfig,
}

impl MacdStochasticTrend {
    pub fn new(config: MacdStochasticTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for MacdStochasticTrend {
    fn name(&self) -> &str {
        "MacdStochasticTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // MACD calculation
        let (macd_line, macd_signal, _) = macd::calculate(
            data,
            self.config.macd_fast_period,
            self.config.macd_slow_period,
            self.config.macd_signal_period,
        )?;
        let macd_arr = macd_line.f64()?;
        let signal_arr = macd_signal.f64()?;

        // Stochastic calculation
        let (stoch_k, stoch_d) = stochastic::calculate(
            data,
            self.config.stoch_k_period,
            3, // standard smoothing for %K
            self.config.stoch_d_period,
        )?;
        let k_arr = stoch_k.f64()?;
        let d_arr = stoch_d.f64()?;

        // ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            let m_curr = macd_arr.get(i);
            let m_prev = macd_arr.get(i - 1);
            let s_curr = signal_arr.get(i);
            let s_prev = signal_arr.get(i - 1);

            let k_curr = k_arr.get(i);
            let k_prev = k_arr.get(i - 1);
            let d_curr = d_arr.get(i);
            let d_prev = d_arr.get(i - 1);

            if let (
                Some(price),
                Some(macd_val),
                Some(sig_val),
                Some(pmacd_val),
                Some(psig_val),
                Some(k),
                Some(d),
                Some(pk),
                Some(pd)
            ) = (
                price_opt, m_curr, s_curr, m_prev, s_prev, k_curr, d_curr, k_prev, d_prev
            ) {
                let macd_bullish = macd_val > sig_val;
                let macd_bearish = macd_val < sig_val;

                let macd_cross_under = pmacd_val >= psig_val && macd_val < sig_val;
                let macd_cross_over = pmacd_val <= psig_val && macd_val > sig_val;

                let stoch_cross_over_oversold = pk <= pd && k > d && k < self.config.stoch_oversold;
                let stoch_cross_under_overbought = pk >= pd && k < d && k > self.config.stoch_overbought;

                let sl_dist = if let Some(atr_val) = atr_opt {
                    atr_val * self.config.stop_loss_atr_mult
                } else {
                    price * 0.05
                };

                let size_hint = format!("{:.4}", self.config.max_position_size);

                // Entry Long
                if macd_bullish && stoch_cross_over_oversold {
                    let stop_loss = price - sl_dist;
                    let take_profit = price + (sl_dist * 2.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: size_hint.clone(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: Some(take_profit),
                        reason: format!("MACD Bullish & Stoch OS Crossover (M:{:.2}, K:{:.2})", macd_val, k),
                        timestamp_ms: timestamp,
                    });
                }

                // Entry Short
                if macd_bearish && stoch_cross_under_overbought {
                    let stop_loss = price + sl_dist;
                    let take_profit = price - (sl_dist * 2.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: size_hint.clone(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: Some(take_profit),
                        reason: format!("MACD Bearish & Stoch OB Crossunder (M:{:.2}, K:{:.2})", macd_val, k),
                        timestamp_ms: timestamp,
                    });
                }

                // Exit Long
                if macd_cross_under || stoch_cross_under_overbought {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("Exit Long Condition Met (M_cross_under: {}, Stoch_OB_crossunder: {})", macd_cross_under, stoch_cross_under_overbought),
                        timestamp_ms: timestamp,
                    });
                }

                // Exit Short
                if macd_cross_over || stoch_cross_over_oversold {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("Exit Short Condition Met (M_cross_over: {}, Stoch_OS_crossover: {})", macd_cross_over, stoch_cross_over_oversold),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: MacdStochasticTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_macd_stoch_trend_signals_long() -> Result<()> {
        let config = MacdStochasticTrendConfig {
            macd_fast_period: 3,
            macd_slow_period: 6,
            macd_signal_period: 3,
            stoch_k_period: 3,
            stoch_d_period: 2,
            stoch_oversold: 80.0, // High threshold to test entry
            stoch_overbought: 20.0, // Low threshold to test exit
            max_position_size: 50.0,
            stop_loss_atr_mult: 1.5,
            atr_period: 3,
            symbol: "TEST".to_string(),
        };
        let strategy = MacdStochasticTrend::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000, 11000, 12000, 13000],
            "high" =>  &[50.0, 40.0, 30.0, 20.0, 10.0, 20.0, 30.0, 40.0, 30.0, 20.0, 25.0, 60.0, 70.0],
            "low" =>   &[40.0, 30.0, 20.0, 10.0, 0.0,  10.0, 20.0, 30.0, 20.0, 10.0, 15.0, 40.0, 50.0],
            "close" => &[45.0, 35.0, 25.0, 15.0, 5.0,  15.0, 25.0, 35.0, 25.0, 15.0, 20.0, 55.0, 60.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let long_entries = signals.iter().filter(|s| s.signal_type == SignalType::Entry && s.side == "buy").count();
        let long_exits = signals.iter().filter(|s| s.signal_type == SignalType::Exit && s.side == "sell").count();

        assert!(long_entries > 0, "Should generate a long entry signal");
        assert!(long_exits > 0, "Should generate a long exit signal");

        // Verify position size is set correctly
        if let Some(entry) = signals.iter().find(|s| s.signal_type == SignalType::Entry && s.side == "buy") {
            assert_eq!(entry.size_hint, "50.0000");
            assert!(entry.stop_loss.is_some());
            assert!(entry.take_profit.is_some());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_macd_stoch_trend_signals_short() -> Result<()> {
        let config = MacdStochasticTrendConfig {
            macd_fast_period: 3,
            macd_slow_period: 6,
            macd_signal_period: 3,
            stoch_k_period: 3,
            stoch_d_period: 2,
            stoch_oversold: 80.0,
            stoch_overbought: 20.0,
            max_position_size: 50.0,
            stop_loss_atr_mult: 1.5,
            atr_period: 3,
            symbol: "TEST".to_string(),
        };
        let strategy = MacdStochasticTrend::new(config);

        // Inverse
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000, 11000, 12000, 13000],
            "high" =>  &[ 0.0, 10.0, 20.0, 30.0, 40.0, 30.0, 20.0, 10.0, 20.0, 30.0, 25.0, -10.0, -20.0],
            "low" =>   &[ 10.0, 20.0, 30.0, 40.0, 50.0, 40.0, 30.0, 20.0, 30.0, 40.0, 35.0,  10.0,  0.0],
            "close" => &[ 5.0, 15.0, 25.0, 35.0, 45.0, 35.0, 25.0, 15.0, 25.0, 35.0, 30.0, -5.0, -10.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let short_entries = signals.iter().filter(|s| s.signal_type == SignalType::Entry && s.side == "sell").count();
        let short_exits = signals.iter().filter(|s| s.signal_type == SignalType::Exit && s.side == "buy").count();

        assert!(short_entries > 0, "Should generate a short entry signal");
        assert!(short_exits > 0, "Should generate a short exit signal");

        if let Some(entry) = signals.iter().find(|s| s.signal_type == SignalType::Entry && s.side == "sell") {
            assert_eq!(entry.size_hint, "50.0000");
            assert!(entry.stop_loss.is_some());
            assert!(entry.take_profit.is_some());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_macd_stoch_trend_empty_data() -> Result<()> {
        let config = MacdStochasticTrendConfig::default();
        let strategy = MacdStochasticTrend::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[] as &[i64],
            "high" => &[] as &[f64],
            "low" => &[] as &[f64],
            "close" => &[] as &[f64]
        )?;

        let signals = strategy.generate_signals(&df).await;
        // MACD calculation should fail with not enough data
        assert!(signals.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let mut strategy = MacdStochasticTrend::new(MacdStochasticTrendConfig::default());
        let new_params = serde_json::json!({
            "macd_fast_period": 10,
            "macd_slow_period": 20,
            "macd_signal_period": 8,
            "stoch_k_period": 10,
            "stoch_d_period": 3,
            "stoch_oversold": 25.0,
            "stoch_overbought": 75.0,
            "max_position_size": 200.0,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "ETHUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.macd_fast_period, 10);
        assert_eq!(strategy.config.stoch_oversold, 25.0);
        assert_eq!(strategy.config.symbol, "ETHUSD");

        Ok(())
    }
}
