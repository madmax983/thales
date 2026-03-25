//! Fisher Transform Reversal Strategy
//!
//! The Fisher Transform Reversal strategy converts price data into a Gaussian normal
//! distribution. Standard price action doesn't have a normal distribution, creating noise.
//! The Fisher Transform highlights real reversals clearly, enabling the strategy to
//! identify potential turning points and trends.
//!
//! # Strategy Logic
//! - **Buy Signal**: Triggered when the Fisher Transform crosses *above* the
//!   oversold threshold (default: -1.5) and its previous value.
//! - **Sell Signal**: Triggered when the Fisher Transform crosses *below* the
//!   overbought threshold (default: 1.5) and its previous value.
//! - **Stop Loss**: Set dynamically using the Average True Range (ATR) multiplied
//!   by a configurable multiplier.

use crate::indicators::{atr, fisher_transform};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration for the Fisher Transform Reversal strategy.
///
/// # Examples
/// ```
/// use strategies::fisher_transform_reversal::FisherTransformReversalConfig;
///
/// let json = r#"{
///     "period": 9,
///     "overbought_threshold": 1.5,
///     "oversold_threshold": -1.5,
///     "atr_period": 14,
///     "atr_multiplier": 2.0,
///     "max_position_size": 100.0,
///     "symbol": "BTCUSD"
/// }"#;
///
/// let config: FisherTransformReversalConfig = serde_json::from_str(json).unwrap();
/// assert_eq!(config.period, 9);
/// assert_eq!(config.overbought_threshold, 1.5);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FisherTransformReversalConfig {
    /// Period for Fisher Transform (typically 9)
    pub period: usize,
    /// Level considered as overbought, typically +1.5 to +2.0
    pub overbought_threshold: f64,
    /// Level considered as oversold, typically -1.5 to -2.0
    pub oversold_threshold: f64,
    /// Period for Average True Range (typically 14)
    pub atr_period: usize,
    /// Multiplier for ATR to set stop loss
    pub atr_multiplier: f64,
    /// Maximum position size
    pub max_position_size: f64,
    /// The asset to trade
    pub symbol: String,
}

impl Default for FisherTransformReversalConfig {
    fn default() -> Self {
        Self {
            period: 9,
            overbought_threshold: 1.5,
            oversold_threshold: -1.5,
            atr_period: 14,
            atr_multiplier: 2.0,
            max_position_size: 1.0,
            symbol: "BTCUSD".to_string(),
        }
    }
}

impl StrategyConfig for FisherTransformReversalConfig {}

/// Fisher Transform Reversal Strategy implementation.
///
/// This strategy utilizes the Fisher Transform indicator to find major price reversals
/// by identifying overbought and oversold extremes.
///
/// # Examples
/// ```
/// use strategies::fisher_transform_reversal::{FisherTransformReversal, FisherTransformReversalConfig};
/// use strategies::strategy::Strategy;
///
/// let config = FisherTransformReversalConfig::default();
/// let strategy = FisherTransformReversal::new(config).unwrap();
///
/// assert_eq!(strategy.name(), "FisherTransformReversal");
/// ```
pub struct FisherTransformReversal {
    config: FisherTransformReversalConfig,
}

impl FisherTransformReversal {
    /// Creates a new `FisherTransformReversal` strategy.
    ///
    /// # Errors
    /// Returns an error if the period is 0, if the overbought threshold is less than
    /// or equal to the oversold threshold, or if the ATR period is 0.
    pub fn new(config: FisherTransformReversalConfig) -> Result<Self> {
        if config.period == 0 {
            anyhow::bail!("Period must be greater than 0");
        }
        if config.overbought_threshold <= config.oversold_threshold {
            anyhow::bail!("Overbought threshold must be > oversold threshold");
        }
        if config.atr_period == 0 {
            anyhow::bail!("ATR period must be > 0");
        }

        Ok(Self { config })
    }
}

#[async_trait]
impl Strategy for FisherTransformReversal {
    fn name(&self) -> &str {
        "FisherTransformReversal"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.period + 1 {
            return Ok(vec![]);
        }

        let fisher_series = fisher_transform::calculate(data, self.config.period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let fisher = fisher_series.f64()?;
        let atr = atr_series.f64()?;
        let close = data.column("close")?.f64()?;
        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let mut in_long = false;
        let mut in_short = false;

        for i in 1..data.height() {
            let curr_fisher = fisher.get(i);
            let prev_fisher = fisher.get(i - 1);
            let curr_close = close.get(i);
            let curr_atr = atr.get(i);
            let ts = timestamps.get(i).unwrap_or(0);

            if let (Some(c_fisher), Some(p_fisher), Some(price), Some(atr_val)) =
                (curr_fisher, prev_fisher, curr_close, curr_atr)
            {
                // Long Entry: Fisher crosses above oversold and signal line (previous)
                let long_entry = c_fisher > p_fisher && p_fisher <= self.config.oversold_threshold;
                // Short Entry: Fisher crosses below overbought and signal line
                let short_entry =
                    c_fisher < p_fisher && p_fisher >= self.config.overbought_threshold;

                let stop_loss_dist = atr_val * self.config.atr_multiplier;

                // Exit conditions
                // Long Exit: Fisher goes above 0 or turns down
                let long_exit = in_long
                    && (c_fisher > 0.0
                        || (c_fisher < p_fisher && c_fisher > self.config.overbought_threshold));
                let short_exit = in_short
                    && (c_fisher < 0.0
                        || (c_fisher > p_fisher && c_fisher < self.config.oversold_threshold));

                if long_exit {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 1.0,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Fisher crossed above 0 or turned down (val: {:.2})",
                            c_fisher
                        ),
                        timestamp_ms: ts,
                    });
                    in_long = false;
                } else if short_exit {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 1.0,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Fisher crossed below 0 or turned up (val: {:.2})",
                            c_fisher
                        ),
                        timestamp_ms: ts,
                    });
                    in_short = false;
                } else if long_entry && !in_long {
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 1.0,
                        stop_loss: Some(price - stop_loss_dist),
                        take_profit: None, // Could add dynamic TP
                        reason: format!(
                            "Fisher crossed above oversold ({:.2} -> {:.2})",
                            p_fisher, c_fisher
                        ),
                        timestamp_ms: ts,
                    });
                    in_long = true;
                    in_short = false; // Simple reverse logic
                } else if short_entry && !in_short {
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 1.0,
                        stop_loss: Some(price + stop_loss_dist),
                        take_profit: None,
                        reason: format!(
                            "Fisher crossed below overbought ({:.2} -> {:.2})",
                            p_fisher, c_fisher
                        ),
                        timestamp_ms: ts,
                    });
                    in_short = true;
                    in_long = false;
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        self.config = serde_json::from_value(params)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parameter_validation() {
        let config1 = FisherTransformReversalConfig {
            period: 0,
            ..Default::default()
        };
        assert!(FisherTransformReversal::new(config1).is_err());

        let config2 = FisherTransformReversalConfig {
            overbought_threshold: -2.0,
            oversold_threshold: 2.0,
            ..Default::default()
        };
        assert!(FisherTransformReversal::new(config2).is_err());
    }

    #[tokio::test]
    async fn test_signal_generation() {
        // Create synthetic data with a clear reversal pattern
        // We need a decent length of data to allow Fisher Transform and ATR to compute
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut closes = Vec::new();
        let mut ts = Vec::new();

        // Downward trend to oversold, then reversal up
        let mut price = 100.0;
        for i in 0..50 {
            if i < 25 {
                price -= 2.0;
            } else {
                price += 2.0;
            }
            highs.push(price + 1.0);
            lows.push(price - 1.0);
            closes.push(price);
            ts.push(i as i64 * 1000);
        }

        let df = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes,
            "timestamp_unix_ms" => ts
        )
        .unwrap();

        let config = FisherTransformReversalConfig {
            period: 9,
            oversold_threshold: -1.0, // less extreme for test
            overbought_threshold: 1.0,
            atr_period: 14,
            atr_multiplier: 2.0,
            max_position_size: 1.0,
            symbol: "TEST".to_string(),
        };

        let strategy = FisherTransformReversal::new(config).unwrap();
        let signals = strategy.generate_signals(&df).await.unwrap();

        assert!(!signals.is_empty(), "Should generate signals");

        let first_entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(first_entry.is_some(), "Should have an entry signal");
        assert_eq!(
            first_entry.unwrap().side,
            "buy",
            "First entry should be buy due to downward trend reversal"
        );
    }
}
