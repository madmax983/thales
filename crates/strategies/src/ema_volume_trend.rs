//! EMA Volume Trend Trading Strategy
//!
//! A trend-following strategy that confirms EMA crossovers with volume expansion.
//! Filtering EMA crossovers with volume helps avoid false breakouts in low-liquidity environments.
//!
//! # Entry Conditions
//! - **Long Entry:** Short EMA crosses above Long EMA AND current volume > Volume SMA.
//! - **Short Entry:** Short EMA crosses below Long EMA AND current volume > Volume SMA.
//!
//! # Exit Conditions
//! - **Long Exit:** Short EMA crosses below Long EMA.
//! - **Short Exit:** Short EMA crosses above Long EMA.

use crate::indicators::{atr, ema, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration for the EMA Volume Trend Strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaVolumeTrendConfig {
    /// The number of periods for the short EMA. (e.g., 9)
    pub short_ema_period: usize,
    /// The number of periods for the long EMA. (e.g., 21)
    pub long_ema_period: usize,
    /// The number of periods for the volume SMA. (e.g., 20)
    pub volume_sma_period: usize,
    /// Maximum position size
    pub max_position_size: f64,
    /// The multiplier for ATR to calculate the stop-loss distance from the entry price.
    pub stop_loss_atr_mult: f64,
    /// The period for calculating the Average True Range (ATR).
    pub atr_period: usize,
    /// The trading pair symbol the strategy is applied to.
    pub symbol: String,
}

impl Default for EmaVolumeTrendConfig {
    fn default() -> Self {
        Self {
            short_ema_period: 9,
            long_ema_period: 21,
            volume_sma_period: 20,
            max_position_size: 100.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for EmaVolumeTrendConfig {}

/// EMA Volume Trend Strategy
pub struct EmaVolumeTrend {
    config: EmaVolumeTrendConfig,
}

impl EmaVolumeTrend {
    /// Creates a new instance of the EMA Volume Trend Strategy.
    pub fn new(config: EmaVolumeTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for EmaVolumeTrend {
    fn name(&self) -> &str {
        "EmaVolumeTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if self.config.short_ema_period == 0
            || self.config.long_ema_period == 0
            || self.config.volume_sma_period == 0
        {
            anyhow::bail!("Periods must be greater than 0");
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let volume_series = data.column("volume")?.clone();
        let volume_arr = volume_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate Indicators
        let short_ema_series = ema::calculate(data, self.config.short_ema_period)?;
        let short_ema_arr = short_ema_series.f64()?;

        let long_ema_series = ema::calculate(data, self.config.long_ema_period)?;
        let long_ema_arr = long_ema_series.f64()?;

        // Need to pass a dataframe where 'close' is actually 'volume' for sma::calculate to work if it hardcodes 'close'
        // Let's check the sma function if it hardcodes 'close'
        // If it does, we need to clone data and replace 'close' with 'volume'
        // For now, let's look at what sma takes. It only takes data and period. So it probably hardcodes 'close'.
        // Let's create a temporary dataframe for volume.

        let mut data_for_volume = data.clone();
        data_for_volume.replace("close", data.column("volume")?.clone())?;

        let volume_sma_series = sma::calculate(&data_for_volume, self.config.volume_sma_period)?;
        let volume_sma_arr = volume_sma_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let short_ema_curr_opt = short_ema_arr.get(i);
            let short_ema_prev_opt = short_ema_arr.get(i - 1);
            let long_ema_curr_opt = long_ema_arr.get(i);
            let long_ema_prev_opt = long_ema_arr.get(i - 1);

            let volume_curr_opt = volume_arr.get(i);
            let volume_sma_curr_opt = volume_sma_arr.get(i);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(short_ema),
                Some(p_short_ema),
                Some(long_ema),
                Some(p_long_ema),
                Some(volume),
                Some(volume_sma),
                Some(price),
            ) = (
                short_ema_curr_opt,
                short_ema_prev_opt,
                long_ema_curr_opt,
                long_ema_prev_opt,
                volume_curr_opt,
                volume_sma_curr_opt,
                price_opt,
            ) {
                let ema_cross_above = p_short_ema <= p_long_ema && short_ema > long_ema;
                let ema_cross_below = p_short_ema >= p_long_ema && short_ema < long_ema;
                let volume_confirmation = volume > volume_sma;

                // Stop loss calculation
                let sl_dist = if let Some(atr_val) = atr_opt {
                    atr_val * self.config.stop_loss_atr_mult
                } else {
                    price * 0.05 // Fallback 5%
                };

                let size_hint = format!("{:.4}", self.config.max_position_size);

                // Long Entry
                if ema_cross_above && volume_confirmation {
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
                        reason: format!(
                            "EMA Volume Trend Long Entry (Short EMA:{:.2}, Long EMA:{:.2}, Vol:{:.2}, Vol SMA:{:.2})",
                            short_ema, long_ema, volume, volume_sma
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Entry
                if ema_cross_below && volume_confirmation {
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
                        reason: format!(
                            "EMA Volume Trend Short Entry (Short EMA:{:.2}, Long EMA:{:.2}, Vol:{:.2}, Vol SMA:{:.2})",
                            short_ema, long_ema, volume, volume_sma
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit
                if ema_cross_below {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "EMA Volume Trend Long Exit (Short EMA:{:.2}, Long EMA:{:.2})",
                            short_ema, long_ema
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Exit
                if ema_cross_above {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "EMA Volume Trend Short Exit (Short EMA:{:.2}, Long EMA:{:.2})",
                            short_ema, long_ema
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: EmaVolumeTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_ema_volume_trend_signals() -> Result<()> {
        let config = EmaVolumeTrendConfig {
            short_ema_period: 2,
            long_ema_period: 3,
            volume_sma_period: 2,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = EmaVolumeTrend::new(config);

        // Needs enough data points for indicators to prime.
        // Let's create a clearer crossover.
        // Short EMA: 2, Long EMA: 3, Vol SMA: 2
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000],
            "high" =>  &[10.0, 10.0, 10.0, 20.0, 25.0, 30.0, 35.0],
            "low" =>   &[ 5.0,  5.0,  5.0, 10.0, 15.0, 20.0, 25.0],
            "close" => &[ 7.0,  7.0,  7.0, 18.0, 22.0, 28.0, 32.0],
            "volume" => &[10.0, 10.0, 10.0, 50.0, 60.0, 70.0, 80.0]
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
        let config = EmaVolumeTrendConfig {
            short_ema_period: 0, // Invalid
            long_ema_period: 3,
            volume_sma_period: 2,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = EmaVolumeTrend::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64],
            "high" =>  &[100.0],
            "low" =>   &[ 90.0],
            "close" => &[ 95.0],
            "volume" => &[ 10.0]
        )?;

        let result = strategy.generate_signals(&df).await;
        assert!(result.is_err(), "Should fail with invalid periods");

        Ok(())
    }
}
