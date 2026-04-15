//! The Schaff Trend Cycle Strategy
//!
//! Uses the STC indicator to identify fast market cycles.
//!
use crate::indicators::{atr, stc};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

use serde::{Deserialize, Serialize};

/// Configuration parameters for the `SchaffTrendCycle` strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchaffTrendCycleConfig {
    /// MACD Fast EMA period (typically 23)
    pub fast_period: usize,
    /// MACD Slow EMA period (typically 50)
    pub slow_period: usize,
    /// Stochastic Cycle period (typically 10)
    pub cycle_period: usize,
    /// Stochastic smoothing period (typically 3)
    pub d_period: usize,
    /// Oversold threshold (typically 25)
    pub oversold_threshold: f64,
    /// Overbought threshold (typically 75)
    pub overbought_threshold: f64,
    /// ATR period for stop loss calculation
    pub atr_period: usize,
    /// ATR multiplier for stop loss
    pub stop_loss_atr_mult: f64,
    /// Trading symbol
    pub symbol: String,
}

impl StrategyConfig for SchaffTrendCycleConfig {}

impl Default for SchaffTrendCycleConfig {
    fn default() -> Self {
        Self {
            fast_period: 23,
            slow_period: 50,
            cycle_period: 10,
            d_period: 3,
            oversold_threshold: 25.0,
            overbought_threshold: 75.0,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "BTCUSD".to_string(),
        }
    }
}

/// Schaff Trend Cycle (STC) Trading Strategy.
///
/// This strategy uses the STC indicator to generate buy and sell signals.
/// A buy signal occurs when the STC line crosses above the oversold threshold (momentum turning up).
/// A sell signal occurs when the STC line crosses below the overbought threshold (momentum turning down).
pub struct SchaffTrendCycle {
    config: SchaffTrendCycleConfig,
}

impl SchaffTrendCycle {
    pub fn new(config: SchaffTrendCycleConfig) -> Self {
        // Validation should ideally happen before instantiation or return Result.
        // For standard signature, we handle invalid configs gracefully or panic in new if fatal.
        // Here we ensure parameters are somewhat sane to prevent runtime panics in calculations.
        let mut cfg = config;
        if cfg.fast_period >= cfg.slow_period {
            cfg.fast_period = cfg.slow_period.saturating_sub(1);
        }
        if cfg.fast_period == 0 {
            cfg.fast_period = 1;
        }
        if cfg.slow_period == 0 {
            cfg.slow_period = 2;
        }
        if cfg.cycle_period == 0 {
            cfg.cycle_period = 1;
        }
        Self { config: cfg }
    }
}

#[async_trait]
impl Strategy for SchaffTrendCycle {
    fn name(&self) -> &str {
        "SchaffTrendCycle"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate STC
        let stc_series = stc::calculate(
            data,
            self.config.fast_period,
            self.config.slow_period,
            self.config.cycle_period,
            self.config.d_period,
        )?;
        let stc_arr = stc_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let stc_curr_opt = stc_arr.get(i);
            let stc_prev_opt = stc_arr.get(i - 1);
            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (Some(stc_c), Some(stc_p), Some(price)) = (stc_curr_opt, stc_prev_opt, price_opt)
            {
                // Long Entry: STC crosses above oversold_threshold
                let cross_above_oversold = stc_p <= self.config.oversold_threshold
                    && stc_c > self.config.oversold_threshold;

                // Short Entry: STC crosses below overbought_threshold
                let cross_below_overbought = stc_p >= self.config.overbought_threshold
                    && stc_c < self.config.overbought_threshold;

                if cross_above_oversold {
                    let sl_dist = atr_opt.unwrap_or(price * 0.05) * self.config.stop_loss_atr_mult;
                    let sl = price - sl_dist;
                    let tp = price + (sl_dist * 2.0); // Simple 1:2 Risk:Reward

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(), // Can be bounded by max_position_size if configured
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: Some(tp),
                        reason: format!(
                            "STC {:.2} crossed above oversold threshold {:.2}",
                            stc_c, self.config.oversold_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                } else if cross_below_overbought {
                    let sl_dist = atr_opt.unwrap_or(price * 0.05) * self.config.stop_loss_atr_mult;
                    let sl = price + sl_dist;
                    let tp = price - (sl_dist * 2.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: Some(tp),
                        reason: format!(
                            "STC {:.2} crossed below overbought threshold {:.2}",
                            stc_c, self.config.overbought_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Exits (optional, often momentum strategies just reverse, but here we explicitly close if momentum drops back)
                // Long Exit: Momentum turns back down from overbought
                if stc_p >= self.config.overbought_threshold
                    && stc_c < self.config.overbought_threshold
                {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // close long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "STC {:.2} dropped below overbought threshold {:.2}",
                            stc_c, self.config.overbought_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Exit: Momentum turns back up from oversold
                if stc_p <= self.config.oversold_threshold && stc_c > self.config.oversold_threshold
                {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // close short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "STC {:.2} rose above oversold threshold {:.2}",
                            stc_c, self.config.oversold_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: SchaffTrendCycleConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_stc_strategy_signals() -> Result<()> {
        let config = SchaffTrendCycleConfig {
            fast_period: 3,
            slow_period: 6,
            cycle_period: 3,
            d_period: 2,
            oversold_threshold: 25.0,
            overbought_threshold: 75.0,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = SchaffTrendCycle::new(config);

        let mut closes = Vec::new();
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut times = Vec::new();

        // Generate a sine wave to trigger overbought/oversold crosses
        for i in 0..100 {
            let val = 100.0 + (i as f64 * 0.3).sin() * 20.0;
            closes.push(val);
            highs.push(val + 2.0);
            lows.push(val - 2.0);
            times.push(i as i64 * 1000);
        }

        let df = df!(
            "timestamp_unix_ms" => times,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // We should get both Entry and Exit signals
        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        assert!(!entries.is_empty(), "No entry signals generated");
        assert!(!exits.is_empty(), "No exit signals generated");

        // Verify entry properties
        let first_entry = &entries[0];
        assert!(first_entry.stop_loss.is_some());
        assert!(first_entry.take_profit.is_some());

        let mut long_entry = false;
        let mut short_entry = false;
        for e in entries {
            if e.side == "buy" {
                long_entry = true;
            }
            if e.side == "sell" {
                short_entry = true;
            }
        }

        assert!(long_entry, "No long entry generated");
        assert!(short_entry, "No short entry generated");

        Ok(())
    }

    #[test]
    fn test_parameter_validation() {
        let config = SchaffTrendCycleConfig {
            fast_period: 50, // fast >= slow
            slow_period: 20,
            cycle_period: 0, // cycle = 0
            d_period: 3,
            oversold_threshold: 25.0,
            overbought_threshold: 75.0,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };

        let strategy = SchaffTrendCycle::new(config);

        // Assert parameters were corrected to safe values
        assert!(
            strategy.config.fast_period < strategy.config.slow_period,
            "Fast period must be < slow period"
        );
        assert_eq!(strategy.config.fast_period, 19); // 20 - 1
        assert_eq!(strategy.config.cycle_period, 1);
    }
}
