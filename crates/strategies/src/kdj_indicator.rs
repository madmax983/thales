//! KDJ Indicator Trading Strategy
//!
//! A mean-reversion and momentum strategy based on the KDJ indicator. It relies on the fast %K line, slow %D line, and divergence %J line to identify overbought/oversold conditions and trend reversals.
//!
//! # Entry Conditions
//! - **Long Entry:** %J line crosses above 0 (oversold reversal) OR %K crosses above %D while both are below 20.
//! - **Short Entry:** %J line crosses below 100 (overbought reversal) OR %K crosses below %D while both are above 80.
//!
//! # Exit Conditions
//! - **Long Exit:** %J line crosses above 100 OR %K crosses below %D.
//! - **Short Exit:** %J line crosses below 0 OR %K crosses above %D.

use crate::indicators::{atr, kdj};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration for the KDJ Indicator Strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdjIndicatorStrategyConfig {
    /// The number of periods to look back for the highest high and lowest low to calculate the fast %K. (e.g., 9)
    pub k_period: usize,
    /// The number of periods to smooth the fast %K to get the slow %K. (e.g., 3)
    pub k_smoothing: usize,
    /// The number of periods to calculate the moving average of the slow %K to get the %D. (e.g., 3)
    pub d_period: usize,
    /// The lower threshold below which the market is considered oversold. (e.g., 20)
    pub oversold_threshold: f64,
    /// The upper threshold above which the market is considered overbought. (e.g., 80)
    pub overbought_threshold: f64,
    /// Maximum position size
    pub max_position_size: f64,
    /// The multiplier for ATR to calculate the stop-loss distance from the entry price.
    pub stop_loss_atr_mult: f64,
    /// The period for calculating the Average True Range (ATR).
    pub atr_period: usize,
    /// The trading pair symbol the strategy is applied to.
    pub symbol: String,
}

impl StrategyConfig for KdjIndicatorStrategyConfig {}

/// KDJ Indicator Strategy
pub struct KdjIndicatorStrategy {
    config: KdjIndicatorStrategyConfig,
}

impl KdjIndicatorStrategy {
    /// Creates a new instance of the KDJ Indicator Strategy.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use strategies::kdj_indicator::{KdjIndicatorStrategy, KdjIndicatorStrategyConfig};
    ///
    /// let config = KdjIndicatorStrategyConfig {
    ///     k_period: 9,
    ///     k_smoothing: 3,
    ///     d_period: 3,
    ///     oversold_threshold: 20.0,
    ///     overbought_threshold: 80.0,
    ///     max_position_size: 1000.0,
    ///     stop_loss_atr_mult: 2.0,
    ///     atr_period: 14,
    ///     symbol: "BTCUSD".to_string(),
    /// };
    ///
    /// let strategy = KdjIndicatorStrategy::new(config);
    /// ```
    pub fn new(config: KdjIndicatorStrategyConfig) -> Self {
        if config.k_period == 0 || config.k_smoothing == 0 || config.d_period == 0 {
            // Logically we should bail here, but constructor can't fail.
            // Using reasonable default or expecting user validation.
            // To properly handle we'd return a Result, but `StrategyFactory` expects new() to return Self.
            // Just return Self and handle in signal generation if needed.
        }
        Self { config }
    }
}

#[async_trait]
impl Strategy for KdjIndicatorStrategy {
    fn name(&self) -> &str {
        "KDJ Indicator Trading Strategy"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if self.config.k_period == 0 || self.config.k_smoothing == 0 || self.config.d_period == 0 {
            anyhow::bail!("Periods must be greater than 0");
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate KDJ
        let (k_series, d_series, j_series) = kdj::calculate(
            data,
            self.config.k_period,
            self.config.k_smoothing,
            self.config.d_period,
        )?;
        let k_arr = k_series.f64()?;
        let d_arr = d_series.f64()?;
        let j_arr = j_series.f64()?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let k_curr_opt = k_arr.get(i);
            let d_curr_opt = d_arr.get(i);
            let j_curr_opt = j_arr.get(i);
            let k_prev_opt = k_arr.get(i - 1);
            let d_prev_opt = d_arr.get(i - 1);
            let j_prev_opt = j_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (Some(k), Some(d), Some(j), Some(pk), Some(pd), Some(pj), Some(price)) = (
                k_curr_opt, d_curr_opt, j_curr_opt, k_prev_opt, d_prev_opt, j_prev_opt, price_opt,
            ) {
                let j_cross_above_0 = pj <= 0.0 && j > 0.0;
                let k_cross_above_d = pk <= pd && k > d;
                let k_d_below_oversold =
                    k < self.config.oversold_threshold && d < self.config.oversold_threshold;

                let j_cross_below_100 = pj >= 100.0 && j < 100.0;
                let k_cross_below_d = pk >= pd && k < d;
                let k_d_above_overbought =
                    k > self.config.overbought_threshold && d > self.config.overbought_threshold;

                let j_cross_above_100 = pj <= 100.0 && j > 100.0;
                let j_cross_below_0 = pj >= 0.0 && j < 0.0;

                // Stop loss calculation
                let sl_dist = if let Some(atr_val) = atr_opt {
                    atr_val * self.config.stop_loss_atr_mult
                } else {
                    price * 0.05 // Fallback 5%
                };

                let size_hint = format!("{:.4}", self.config.max_position_size);

                // Long Entry
                if j_cross_above_0 || (k_cross_above_d && k_d_below_oversold) {
                    let stop_loss = price - sl_dist;
                    let take_profit = price + (sl_dist * 2.0); // Simple 1:2 RR

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: size_hint.clone(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: Some(take_profit),
                        reason: format!("KDJ Long Entry (J:{:.2}, K:{:.2}, D:{:.2})", j, k, d),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Entry
                if j_cross_below_100 || (k_cross_below_d && k_d_above_overbought) {
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
                        reason: format!("KDJ Short Entry (J:{:.2}, K:{:.2}, D:{:.2})", j, k, d),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit
                if j_cross_above_100 || k_cross_below_d {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("KDJ Long Exit (J:{:.2}, K:{:.2}, D:{:.2})", j, k, d),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Exit
                if j_cross_below_0 || k_cross_above_d {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("KDJ Short Exit (J:{:.2}, K:{:.2}, D:{:.2})", j, k, d),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: KdjIndicatorStrategyConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_long_entry_and_exit() -> Result<()> {
        let config = KdjIndicatorStrategyConfig {
            k_period: 3,
            k_smoothing: 2,
            d_period: 2,
            oversold_threshold: 40.0,
            overbought_threshold: 60.0,
            max_position_size: 50.0,
            stop_loss_atr_mult: 1.5,
            atr_period: 3,
            symbol: "TEST".to_string(),
        };
        let strategy = KdjIndicatorStrategy::new(config);

        // We create data to trigger a long entry and then a long exit.
        // Long entry: %J crosses above 0 OR (%K crosses above %D and both < oversold).
        // Long exit: %J crosses above 100 OR %K crosses below %D.
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000],
            "high" =>  &[50.0, 60.0, 70.0, 80.0, 90.0, 80.0, 70.0, 60.0],
            "low" =>   &[40.0, 50.0, 60.0, 70.0, 80.0, 70.0, 60.0, 50.0],
            "close" => &[45.0, 55.0, 65.0, 75.0, 85.0, 75.0, 65.0, 55.0]
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
    async fn test_short_entry_and_exit() -> Result<()> {
        let config = KdjIndicatorStrategyConfig {
            k_period: 3,
            k_smoothing: 2,
            d_period: 2,
            oversold_threshold: 40.0,
            overbought_threshold: 60.0,
            max_position_size: 50.0,
            stop_loss_atr_mult: 1.5,
            atr_period: 3,
            symbol: "TEST".to_string(),
        };
        let strategy = KdjIndicatorStrategy::new(config);

        // Short entry: %J crosses below 100 OR (%K crosses below %D and both > overbought).
        // Short exit: %J crosses below 0 OR %K crosses above %D.
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000],
            "high" =>  &[50.0, 40.0, 30.0, 20.0, 10.0, 20.0, 30.0, 40.0],
            "low" =>   &[40.0, 30.0, 20.0, 10.0, 0.0,  10.0, 20.0, 30.0],
            "close" => &[45.0, 35.0, 25.0, 15.0, 5.0,  15.0, 25.0, 35.0]
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
    async fn test_no_signals_on_flat_market() -> Result<()> {
        let config = KdjIndicatorStrategyConfig {
            k_period: 3,
            k_smoothing: 2,
            d_period: 2,
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = KdjIndicatorStrategy::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "high" =>  &[100.0, 100.0, 100.0, 100.0, 100.0],
            "low" =>   &[100.0, 100.0, 100.0, 100.0, 100.0],
            "close" => &[100.0, 100.0, 100.0, 100.0, 100.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty(), "Should not generate signals on completely flat data without crossovers");

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let config = KdjIndicatorStrategyConfig {
            k_period: 0, // Invalid
            k_smoothing: 1,
            d_period: 2,
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = KdjIndicatorStrategy::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000],
            "high" =>  &[100.0, 100.0, 100.0],
            "low" =>   &[ 90.0,  90.0,  90.0],
            "close" => &[ 95.0,  95.0,  95.0]
        )?;

        let result = strategy.generate_signals(&df).await;
        assert!(result.is_err(), "Should fail with invalid periods");
        assert_eq!(result.unwrap_err().to_string(), "Periods must be greater than 0");

        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let config = KdjIndicatorStrategyConfig {
            k_period: 9,
            k_smoothing: 3,
            d_period: 3,
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let mut strategy = KdjIndicatorStrategy::new(config);

        let new_params = serde_json::json!({
            "k_period": 14,
            "k_smoothing": 3,
            "d_period": 3,
            "oversold_threshold": 30.0,
            "overbought_threshold": 70.0,
            "max_position_size": 200.0,
            "stop_loss_atr_mult": 2.0,
            "atr_period": 14,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.k_period, 14);
        assert_eq!(strategy.config.oversold_threshold, 30.0);
        assert_eq!(strategy.config.max_position_size, 200.0);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
