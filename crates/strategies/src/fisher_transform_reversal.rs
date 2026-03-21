use crate::indicators::{atr, fisher_transform};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// Fisher Transform Reversal Strategy Configuration
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

pub struct FisherTransformReversal {
    config: FisherTransformReversalConfig,
}

impl FisherTransformReversal {
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
                let short_entry = c_fisher < p_fisher && p_fisher >= self.config.overbought_threshold;

                let stop_loss_dist = atr_val * self.config.atr_multiplier;

                // Exit conditions
                // Long Exit: Fisher goes above 0 or turns down
                let long_exit = in_long && (c_fisher > 0.0 || (c_fisher < p_fisher && c_fisher > self.config.overbought_threshold));
                let short_exit = in_short && (c_fisher < 0.0 || (c_fisher > p_fisher && c_fisher < self.config.oversold_threshold));

                if long_exit {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 1.0,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("Fisher crossed above 0 or turned down (val: {:.2})", c_fisher),
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
                        reason: format!("Fisher crossed below 0 or turned up (val: {:.2})", c_fisher),
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
                        reason: format!("Fisher crossed above oversold ({:.2} -> {:.2})", p_fisher, c_fisher),
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
                        reason: format!("Fisher crossed below overbought ({:.2} -> {:.2})", p_fisher, c_fisher),
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
        let mut config = FisherTransformReversalConfig::default();
        config.period = 0;
        assert!(FisherTransformReversal::new(config).is_err());

        let mut config = FisherTransformReversalConfig::default();
        config.overbought_threshold = -2.0;
        config.oversold_threshold = 2.0;
        assert!(FisherTransformReversal::new(config).is_err());
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
            if i < 25 { price -= 2.0; } else { price += 2.0; }
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
        ).unwrap();

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
        assert_eq!(first_entry.unwrap().side, "buy", "First entry should be buy due to downward trend reversal");
    }
}