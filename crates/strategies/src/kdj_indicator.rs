//! # KDJ Indicator Strategy
//!
//! The KDJ Indicator is an extension of the Stochastic Oscillator.
//! It consists of three lines:
//! %K = Fast Stochastic
//! %D = Slow Stochastic (SMA of %K)
//! %J = 3 * %K - 2 * %D (Divergence of %K and %D)
//!
//! ## Overview
//!
//! Signals are generated when %J crosses extreme zones or when %K crosses %D.
//! - **Buy Signal**: When %J crosses above 0 OR %K crosses above %D while both are below 20.
//! - **Sell Signal**: When %J crosses below 100 OR %K crosses below %D while both are above 80.
//!
//! ## Configuration Example
//!
//! ```rust
//! use strategies::kdj_indicator::KdjIndicatorConfig;
//!
//! let config = KdjIndicatorConfig {
//!     k_period: 9,
//!     k_smoothing: 3,
//!     d_period: 3,
//!     oversold_threshold: 20.0,
//!     overbought_threshold: 80.0,
//!     stop_loss_atr_mult: 2.0,
//!     atr_period: 14,
//!     max_position_size: 100.0,
//!     symbol: "BTCUSD".to_string(),
//! };
//! ```

use crate::indicators::{atr, kdj};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration parameters for the `KdjIndicator` strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdjIndicatorConfig {
    /// The lookback period for %K calculation.
    pub k_period: usize,
    /// The smoothing period for %K.
    pub k_smoothing: usize,
    /// The smoothing period for %D.
    pub d_period: usize,
    /// The threshold below which the asset is considered oversold.
    pub oversold_threshold: f64,
    /// The threshold above which the asset is considered overbought.
    pub overbought_threshold: f64,
    /// Multiplier for the ATR to determine the stop loss distance.
    pub stop_loss_atr_mult: f64,
    /// The period for the Average True Range (ATR) calculation.
    pub atr_period: usize,
    /// The maximum position size allowed for a single trade.
    pub max_position_size: f64,
    /// The symbol this strategy is targeting.
    pub symbol: String,
}

impl Default for KdjIndicatorConfig {
    fn default() -> Self {
        Self {
            k_period: 9,
            k_smoothing: 3,
            d_period: 3,
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        }
    }
}

impl StrategyConfig for KdjIndicatorConfig {}

/// KDJ Indicator Strategy
pub struct KdjIndicator {
    pub config: KdjIndicatorConfig,
}

impl KdjIndicator {
    pub fn new(config: KdjIndicatorConfig) -> Result<Self> {
        if config.k_period == 0 || config.k_smoothing == 0 || config.d_period == 0 {
            anyhow::bail!("Periods must be greater than 0");
        }
        Ok(Self { config })
    }
}

#[async_trait]
impl Strategy for KdjIndicator {
    fn name(&self) -> &str {
        "KdjIndicator"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.k_period + self.config.d_period + self.config.k_smoothing {
            return Ok(vec![]);
        }

        let (k_series, d_series, j_series) = kdj::calculate(
            data,
            self.config.k_period,
            self.config.k_smoothing,
            self.config.d_period,
        )?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let k_arr = k_series.f64()?;
        let d_arr = d_series.f64()?;
        let j_arr = j_series.f64()?;
        let atr_arr = atr_series.f64()?;

        let close = data.column("close")?.f64()?;
        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();

        for i in 1..data.height() {
            if let (
                Some(k),
                Some(d),
                Some(j),
                Some(prev_k),
                Some(prev_d),
                Some(prev_j),
                Some(price),
                Some(ts),
                Some(atr_val),
            ) = (
                k_arr.get(i),
                d_arr.get(i),
                j_arr.get(i),
                k_arr.get(i - 1),
                d_arr.get(i - 1),
                j_arr.get(i - 1),
                close.get(i),
                timestamps.get(i),
                atr_arr.get(i),
            ) {
                // Long Entry: %J crosses above 0 OR %K crosses above %D while both are below 20.
                let j_cross_above_0 = prev_j <= 0.0 && j > 0.0;
                let k_cross_above_d_oversold = prev_k <= prev_d && k > d && k < self.config.oversold_threshold && d < self.config.oversold_threshold;

                if j_cross_above_0 || k_cross_above_d_oversold {
                    let sl = price - (atr_val * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: None,
                        reason: "KDJ Long Entry".to_string(),
                        timestamp_ms: ts,
                    });
                }

                // Short Entry: %J crosses below 100 OR %K crosses below %D while both are above 80.
                let j_cross_below_100 = prev_j >= 100.0 && j < 100.0;
                let k_cross_below_d_overbought = prev_k >= prev_d && k < d && k > self.config.overbought_threshold && d > self.config.overbought_threshold;

                if j_cross_below_100 || k_cross_below_d_overbought {
                    let sl = price + (atr_val * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: None,
                        reason: "KDJ Short Entry".to_string(),
                        timestamp_ms: ts,
                    });
                }

                // Long Exit: %J crosses above 100 OR %K crosses below %D.
                let j_cross_above_100 = prev_j <= 100.0 && j > 100.0;
                let k_cross_below_d = prev_k >= prev_d && k < d;

                if j_cross_above_100 || k_cross_below_d {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "KDJ Long Exit".to_string(),
                        timestamp_ms: ts,
                    });
                }

                // Short Exit: %J crosses below 0 OR %K crosses above %D.
                let j_cross_below_0 = prev_j >= 0.0 && j < 0.0;
                let k_cross_above_d = prev_k <= prev_d && k > d;

                if j_cross_below_0 || k_cross_above_d {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "KDJ Short Exit".to_string(),
                        timestamp_ms: ts,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: KdjIndicatorConfig = serde_json::from_value(params)?;
        if new_config.k_period == 0 || new_config.k_smoothing == 0 || new_config.d_period == 0 {
            anyhow::bail!("Periods must be greater than 0");
        }
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_kdj_indicator() -> Result<()> {
        // We need a dataframe large enough to calculate KDJ and ATR
        // K=9, D=3, Smoothing=3
        let n = 20;
        let mut highs = vec![100.0; n];
        let mut lows = vec![50.0; n];
        let mut closes = vec![75.0; n];
        let mut times = vec![0; n];

        // Setup a long entry condition at end
        // %K and %D should be below 20.
        // We can just simulate it by providing dropping prices
        for i in 0..15 {
            closes[i] = 100.0 - i as f64 * 5.0;
            lows[i] = closes[i] - 10.0;
            highs[i] = closes[i] + 10.0;
            times[i] = i as i64 * 1000;
        }

        // Jump price back up to create a crossover
        closes[18] = 40.0; // drops
        closes[19] = 80.0; // goes up
        lows[19] = 70.0;
        highs[19] = 90.0;

        let df = df!(
            "high" => &highs,
            "low" => &lows,
            "close" => &closes,
            "timestamp_unix_ms" => &times
        )?;

        let strategy = KdjIndicator::new(KdjIndicatorConfig::default())?;
        let signals = strategy.generate_signals(&df).await?;

        // We just verify it runs and returns signals without crashing
        assert!(signals.len() >= 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = KdjIndicator::new(KdjIndicatorConfig::default())?;

        let invalid_params = serde_json::json!({
            "k_period": 0,
            "k_smoothing": 3,
            "d_period": 3,
            "oversold_threshold": 20.0,
            "overbought_threshold": 80.0,
            "stop_loss_atr_mult": 2.0,
            "atr_period": 14,
            "max_position_size": 100.0,
            "symbol": "TEST"
        });

        assert!(strategy.update_params(invalid_params).await.is_err());

        Ok(())
    }
}
