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

        let time_series = data.column("timestamp_unix_ms")?;

        // Calculate KDJ
        let (k_series, d_series, j_series) = kdj::calculate(
            data,
            self.config.k_period,
            self.config.k_smoothing,
            self.config.d_period,
        )?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        // Create DataFrame to compute conditions efficiently
        let mut df = DataFrame::new(vec![
            time_series.clone(),
            close_series.clone(),
            k_series.clone(),
            d_series.clone(),
            j_series.clone(),
            atr_series.clone(),
        ])?;

        // Rename columns for easier reference if needed
        df.rename("timestamp_unix_ms", "time")?;
        df.rename("close", "price")?;
        df.rename("atr", "atr")?;

        let lazy_df = df.lazy();

        // Define conditions
        let j_cross_above_0 = col("kdj_j")
            .shift(lit(1))
            .lt_eq(lit(0.0))
            .and(col("kdj_j").gt(lit(0.0)));
        let k_cross_above_d = col("kdj_k")
            .shift(lit(1))
            .lt_eq(col("kdj_d").shift(lit(1)))
            .and(col("kdj_k").gt(col("kdj_d")));
        let k_d_below_oversold = col("kdj_k")
            .lt(lit(self.config.oversold_threshold))
            .and(col("kdj_d").lt(lit(self.config.oversold_threshold)));

        let j_cross_below_100 = col("kdj_j")
            .shift(lit(1))
            .gt_eq(lit(100.0))
            .and(col("kdj_j").lt(lit(100.0)));
        let k_cross_below_d = col("kdj_k")
            .shift(lit(1))
            .gt_eq(col("kdj_d").shift(lit(1)))
            .and(col("kdj_k").lt(col("kdj_d")));
        let k_d_above_overbought = col("kdj_k")
            .gt(lit(self.config.overbought_threshold))
            .and(col("kdj_d").gt(lit(self.config.overbought_threshold)));

        let j_cross_above_100 = col("kdj_j")
            .shift(lit(1))
            .lt_eq(lit(100.0))
            .and(col("kdj_j").gt(lit(100.0)));
        let j_cross_below_0 = col("kdj_j")
            .shift(lit(1))
            .gt_eq(lit(0.0))
            .and(col("kdj_j").lt(lit(0.0)));

        let long_entry_cond = j_cross_above_0
            .clone()
            .or(k_cross_above_d.clone().and(k_d_below_oversold));
        let short_entry_cond = j_cross_below_100
            .clone()
            .or(k_cross_below_d.clone().and(k_d_above_overbought));
        let long_exit_cond = j_cross_above_100.clone().or(k_cross_below_d.clone());
        let short_exit_cond = j_cross_below_0.clone().or(k_cross_above_d.clone());

        let res_df = lazy_df
            .with_columns(vec![
                long_entry_cond.alias("long_entry"),
                short_entry_cond.alias("short_entry"),
                long_exit_cond.alias("long_exit"),
                short_exit_cond.alias("short_exit"),
            ])
            .collect()?;

        let mut signals = Vec::new();

        let times = res_df.column("time")?.i64()?;
        let prices = res_df.column("price")?.f64()?;
        let atrs = res_df.column("atr")?.f64()?;
        let ks = res_df.column("kdj_k")?.f64()?;
        let ds = res_df.column("kdj_d")?.f64()?;
        let js = res_df.column("kdj_j")?.f64()?;

        let long_entries = res_df.column("long_entry")?.bool()?;
        let short_entries = res_df.column("short_entry")?.bool()?;
        let long_exits = res_df.column("long_exit")?.bool()?;
        let short_exits = res_df.column("short_exit")?.bool()?;

        for i in 0..times.len() {
            // Check signals for each row
            let is_long_entry = long_entries.get(i).unwrap_or(false);
            let is_short_entry = short_entries.get(i).unwrap_or(false);
            let is_long_exit = long_exits.get(i).unwrap_or(false);
            let is_short_exit = short_exits.get(i).unwrap_or(false);

            if !is_long_entry && !is_short_entry && !is_long_exit && !is_short_exit {
                continue;
            }

            let timestamp = times.get(i).unwrap_or(0);
            let price = if let Some(p) = prices.get(i) {
                p
            } else {
                continue;
            };
            let atr = atrs.get(i).unwrap_or(price * 0.05);
            let k = ks.get(i).unwrap_or(0.0);
            let d = ds.get(i).unwrap_or(0.0);
            let j = js.get(i).unwrap_or(0.0);

            let sl_dist = atr * self.config.stop_loss_atr_mult;
            let size_hint = format!("{:.4}", self.config.max_position_size);

            if is_long_entry {
                signals.push(Signal {
                    signal_type: SignalType::Entry,
                    symbol: self.config.symbol.clone(),
                    side: "buy".to_string(),
                    size_hint: size_hint.clone(),
                    confidence: 0.8,
                    stop_loss: Some(price - sl_dist),
                    take_profit: Some(price + (sl_dist * 2.0)), // Optional Simple 1:2 RR, fallback logic won't fail
                    reason: format!("KDJ Long Entry (J:{:.2}, K:{:.2}, D:{:.2})", j, k, d),
                    timestamp_ms: timestamp,
                });
            }

            if is_short_entry {
                signals.push(Signal {
                    signal_type: SignalType::Entry,
                    symbol: self.config.symbol.clone(),
                    side: "sell".to_string(),
                    size_hint: size_hint.clone(),
                    confidence: 0.8,
                    stop_loss: Some(price + sl_dist),
                    take_profit: Some(price - (sl_dist * 2.0)),
                    reason: format!("KDJ Short Entry (J:{:.2}, K:{:.2}, D:{:.2})", j, k, d),
                    timestamp_ms: timestamp,
                });
            }

            if is_long_exit {
                signals.push(Signal {
                    signal_type: SignalType::Exit,
                    symbol: self.config.symbol.clone(),
                    side: "sell".to_string(),
                    size_hint: "max".to_string(),
                    confidence: 0.8,
                    stop_loss: None,
                    take_profit: None,
                    reason: format!("KDJ Long Exit (J:{:.2}, K:{:.2}, D:{:.2})", j, k, d),
                    timestamp_ms: timestamp,
                });
            }

            if is_short_exit {
                signals.push(Signal {
                    signal_type: SignalType::Exit,
                    symbol: self.config.symbol.clone(),
                    side: "buy".to_string(),
                    size_hint: "max".to_string(),
                    confidence: 0.8,
                    stop_loss: None,
                    take_profit: None,
                    reason: format!("KDJ Short Exit (J:{:.2}, K:{:.2}, D:{:.2})", j, k, d),
                    timestamp_ms: timestamp,
                });
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

    fn get_default_config() -> KdjIndicatorStrategyConfig {
        KdjIndicatorStrategyConfig {
            k_period: 3,
            k_smoothing: 1,
            d_period: 2,
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        }
    }

    #[tokio::test]
    async fn test_kdj_strategy_signals() -> Result<()> {
        let strategy = KdjIndicatorStrategy::new(get_default_config());

        // We construct DataFrame to test crossings.
        // Needs high, low, close, timestamp_unix_ms.
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000],
            "high" =>  &[100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0],
            "low" =>   &[ 90.0,  90.0,  90.0,  90.0,  90.0,  90.0,  90.0],
            "close" => &[ 95.0,  95.0,  91.0,  90.5,  91.5,  95.0,  96.0]
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
    async fn test_empty_data() {
        let strategy = KdjIndicatorStrategy::new(get_default_config());
        let df = DataFrame::default();
        let result = strategy.generate_signals(&df).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut config = get_default_config();
        config.k_period = 0; // Invalid

        let strategy = KdjIndicatorStrategy::new(config);

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

    #[tokio::test]
    async fn test_extreme_volatility() -> Result<()> {
        let strategy = KdjIndicatorStrategy::new(get_default_config());

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "high" =>  &[100.0, 1000.0, 10.0, 10000.0],
            "low" =>   &[ 90.0,   10.0,  1.0,     5.0],
            "close" => &[ 95.0,  500.0,  5.0,  5000.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;
        // Just making sure it doesn't panic and logic completes
        let _len = signals.len();

        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let mut strategy = KdjIndicatorStrategy::new(get_default_config());
        let new_config = KdjIndicatorStrategyConfig {
            k_period: 14,
            k_smoothing: 3,
            d_period: 3,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            max_position_size: 200.0,
            stop_loss_atr_mult: 2.5,
            atr_period: 14,
            symbol: "NEW".to_string(),
        };

        let json_val = serde_json::to_value(new_config)?;
        strategy.update_params(json_val).await?;

        assert_eq!(strategy.config.k_period, 14);
        assert_eq!(strategy.config.oversold_threshold, 30.0);
        assert_eq!(strategy.config.symbol, "NEW");

        Ok(())
    }
}
