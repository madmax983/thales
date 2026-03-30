//! Volume-Weighted MACD Strategy
//!
//! A variation of MACD that weights price by volume, aiming to filter out low-volume false signals.
//! Uses VWMA (Volume Weighted Moving Average) instead of EMA for the fast and slow lines.

use crate::indicators::{atr, ema, vwma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VwMacdConfig {
    pub fast_period: usize,
    pub slow_period: usize,
    pub signal_period: usize,
    pub max_position_size: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for VwMacdConfig {
    fn default() -> Self {
        Self {
            fast_period: 12,
            slow_period: 26,
            signal_period: 9,
            max_position_size: 100.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for VwMacdConfig {}

pub struct VwMacd {
    config: VwMacdConfig,
}

impl VwMacd {
    pub fn new(config: VwMacdConfig) -> Result<Self> {
        if config.fast_period >= config.slow_period {
            anyhow::bail!("Fast period must be less than slow period");
        }
        if config.fast_period == 0 || config.slow_period == 0 || config.signal_period == 0 {
            anyhow::bail!("Periods must be greater than 0");
        }
        Ok(Self { config })
    }
}

#[async_trait]
impl Strategy for VwMacd {
    fn name(&self) -> &str {
        "VwMacd"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.slow_period + self.config.signal_period {
            return Ok(vec![]);
        }

        let fast_vwma = vwma::calculate(data, self.config.fast_period)?;
        let slow_vwma = vwma::calculate(data, self.config.slow_period)?;

        let fast_arr = fast_vwma.f64()?;
        let slow_arr = slow_vwma.f64()?;

        // Calculate MACD line (Fast VWMA - Slow VWMA)
        let mut macd_line_vals: Vec<Option<f64>> = Vec::with_capacity(data.height());
        for i in 0..data.height() {
            if let (Some(f), Some(s)) = (fast_arr.get(i), slow_arr.get(i)) {
                macd_line_vals.push(Some(f - s));
            } else {
                macd_line_vals.push(None);
            }
        }

        let macd_line_series = Series::new("close", macd_line_vals); // Named "close" so `ema::calculate` can use it
        let macd_line_df = DataFrame::new(vec![macd_line_series.clone()])?;

        // Calculate Signal line (EMA of MACD line)
        let signal_line_series = ema::calculate(&macd_line_df, self.config.signal_period)?;
        let signal_line_arr = signal_line_series.f64()?;
        let macd_line_arr = macd_line_series.f64()?;

        let close_series = data.column("close")?;
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));

        let size_hint = self.config.max_position_size.to_string();

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);

            let macd_curr_opt = macd_line_arr.get(i).and_then(Decimal::from_f64_retain);
            let macd_prev_opt = macd_line_arr.get(i - 1).and_then(Decimal::from_f64_retain);
            let sig_curr_opt = signal_line_arr.get(i).and_then(Decimal::from_f64_retain);
            let sig_prev_opt = signal_line_arr
                .get(i - 1)
                .and_then(Decimal::from_f64_retain);
            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(macd_curr), Some(macd_prev), Some(sig_curr), Some(sig_prev), Some(price)) = (
                macd_curr_opt,
                macd_prev_opt,
                sig_curr_opt,
                sig_prev_opt,
                price_opt,
            ) {
                // Bullish Crossover (VW-MACD crosses above Signal)
                if macd_prev <= sig_prev && macd_curr > sig_curr {
                    // Exit any existing Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Buy to cover short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "VW-MACD crossed above Signal (Exit Short)".to_string(),
                        timestamp_ms: timestamp,
                    });

                    // Entry Long filter: only enter if MACD is below 0 (oversold momentum shift)
                    if macd_curr < Decimal::ZERO {
                        let sl = if let Some(atr_val) = atr_opt {
                            price - (atr_val * atr_mult_dec)
                        } else {
                            price * Decimal::from_f64_retain(0.95).unwrap()
                        };

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: size_hint.clone(),
                            confidence: 0.8,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: None,
                            reason: format!(
                                "VW-MACD Bullish Crossover < 0 (Fast: {}, Slow: {}, Signal: {})",
                                self.config.fast_period,
                                self.config.slow_period,
                                self.config.signal_period
                            ),
                            timestamp_ms: timestamp,
                        });
                    }
                }

                // Bearish Crossover (VW-MACD crosses below Signal)
                if macd_prev >= sig_prev && macd_curr < sig_curr {
                    // Exit any existing Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Sell to close long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "VW-MACD crossed below Signal (Exit Long)".to_string(),
                        timestamp_ms: timestamp,
                    });

                    // Entry Short filter: only enter if MACD is above 0 (overbought momentum shift)
                    if macd_curr > Decimal::ZERO {
                        let sl = if let Some(atr_val) = atr_opt {
                            price + (atr_val * atr_mult_dec)
                        } else {
                            price * Decimal::from_f64_retain(1.05).unwrap()
                        };

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: size_hint.clone(),
                            confidence: 0.8,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: None,
                            reason: format!(
                                "VW-MACD Bearish Crossover > 0 (Fast: {}, Slow: {}, Signal: {})",
                                self.config.fast_period,
                                self.config.slow_period,
                                self.config.signal_period
                            ),
                            timestamp_ms: timestamp,
                        });
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: VwMacdConfig = serde_json::from_value(params)?;
        if new_config.fast_period >= new_config.slow_period {
            anyhow::bail!("Fast period must be less than slow period");
        }
        if new_config.fast_period == 0
            || new_config.slow_period == 0
            || new_config.signal_period == 0
        {
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
    async fn test_vw_macd_signals() -> Result<()> {
        let config = VwMacdConfig {
            fast_period: 2,
            slow_period: 4,
            signal_period: 2,
            max_position_size: 100.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = VwMacd::new(config)?;

        // Engineer prices to force an oversold MACD < 0 bullish cross, then overbought MACD > 0 bearish cross
        let mut closes = Vec::new();
        let mut volumes = Vec::new();
        let mut timestamps = Vec::new();
        let mut highs = Vec::new();
        let mut lows = Vec::new();

        for i in 0..20 {
            timestamps.push(i * 1000);
            highs.push(100.0);
            lows.push(100.0);

            if i < 8 {
                // Downtrend to make MACD negative
                closes.push(100.0 - (i as f64 * 5.0));
                volumes.push(1000.0 + (i as f64 * 100.0));
            } else if i < 14 {
                // Sharp Uptrend to cause Bullish Crossover
                closes.push(50.0 + ((i - 8) as f64 * 15.0));
                volumes.push(2000.0);
            } else {
                // Sharp Downtrend to cause Bearish Crossover
                closes.push(140.0 - ((i - 14) as f64 * 15.0));
                volumes.push(2000.0);
            }
        }

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows,
            "volume" => volumes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let long_entry = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "buy");
        assert!(long_entry.is_some(), "Expected Long Entry Signal");

        let short_entry = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "sell");
        assert!(short_entry.is_some(), "Expected Short Entry Signal");

        let long_exit = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Exit && s.side == "sell");
        assert!(long_exit.is_some(), "Expected Long Exit Signal");

        let short_exit = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Exit && s.side == "buy");
        assert!(short_exit.is_some(), "Expected Short Exit Signal");

        Ok(())
    }

    #[tokio::test]
    async fn test_vw_macd_edge_cases() -> Result<()> {
        let config = VwMacdConfig {
            fast_period: 12,
            slow_period: 26,
            signal_period: 9,
            max_position_size: 100.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = VwMacd::new(config)?;

        // Empty dataframe
        let df_empty = DataFrame::default();
        let result = strategy.generate_signals(&df_empty).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());

        // Single row
        let closes = vec![10.0];
        let df_single = df!(
            "timestamp_unix_ms" => vec![1000],
            "close" => closes.clone(),
            "high" => closes.clone(),
            "low" => closes.clone(),
            "volume" => closes.clone()
        )?;
        let signals = strategy.generate_signals(&df_single).await?;
        assert!(signals.is_empty());

        Ok(())
    }

    #[test]
    fn test_vw_macd_parameter_validation() {
        // Fast period >= slow period
        let invalid_config = VwMacdConfig {
            fast_period: 26,
            slow_period: 12,
            signal_period: 9,
            max_position_size: 100.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        assert!(VwMacd::new(invalid_config).is_err());

        // Zero period
        let zero_config = VwMacdConfig {
            fast_period: 0,
            slow_period: 26,
            signal_period: 9,
            max_position_size: 100.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        assert!(VwMacd::new(zero_config).is_err());
    }
}
