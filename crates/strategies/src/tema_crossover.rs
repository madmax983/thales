use crate::indicators::{atr::calculate as calculate_atr, tema::tema};
use crate::strategy::{Signal, SignalType, Strategy, StrategyType};
use anyhow::{Context, Result};
use async_trait::async_trait;
use polars::prelude::*;

pub struct TemaCrossover {
    config: TemaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TemaCrossoverConfig {
    pub fast_period: usize,
    pub slow_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for TemaCrossoverConfig {
    fn default() -> Self {
        Self {
            fast_period: 10,
            slow_period: 30,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl TemaCrossoverConfig {
    pub fn validate(&self) -> Result<()> {
        if self.fast_period == 0 {
            anyhow::bail!("Fast period must be greater than 0");
        }
        if self.slow_period == 0 {
            anyhow::bail!("Slow period must be greater than 0");
        }
        if self.fast_period >= self.slow_period {
            anyhow::bail!("Fast period must be less than slow period");
        }
        if self.atr_period == 0 {
            anyhow::bail!("ATR period must be greater than 0");
        }
        if self.stop_loss_atr_mult <= 0.0 {
            anyhow::bail!("Stop loss ATR multiplier must be positive");
        }
        Ok(())
    }
}

impl TemaCrossover {
    pub fn new(config: TemaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for TemaCrossover {
    fn name(&self) -> &str {
        "TemaCrossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        self.config.validate()?;

        if data.height() < self.config.slow_period + 5 {
            return Ok(vec![]); // Not enough data
        }

        let close = data
            .column("close")
            .context("Missing 'close' column")?
            .f64()
            .context("Close column must be numeric")?;

        // Calculate TEMA
        let close_series = data.column("close")?.clone();
        let fast_tema_series = tema(&close_series, self.config.fast_period)?;
        let slow_tema_series = tema(&close_series, self.config.slow_period)?;

        let fast_tema = fast_tema_series.f64()?;
        let slow_tema = slow_tema_series.f64()?;

        // Calculate ATR
        let atr_series = calculate_atr(data, self.config.atr_period)?;
        let atr = atr_series.f64()?;

        let timestamps = data
            .column("timestamp_unix_ms")
            .context("Missing 'timestamp_unix_ms'")?
            .i64()
            .context("Timestamp must be i64")?;

        let mut signals = Vec::new();
        let mut in_position = false;
        let mut position_side = "";

        for i in 1..data.height() {
            let prev_fast = fast_tema.get(i - 1);
            let prev_slow = slow_tema.get(i - 1);
            let curr_fast = fast_tema.get(i);
            let curr_slow = slow_tema.get(i);
            let current_close = close.get(i);
            let current_atr = atr.get(i);

            // Need all data points
            if let (
                Some(p_fast),
                Some(p_slow),
                Some(c_fast),
                Some(c_slow),
                Some(c_close),
                Some(c_atr),
            ) = (
                prev_fast,
                prev_slow,
                curr_fast,
                curr_slow,
                current_close,
                current_atr,
            ) {
                let ts = timestamps.get(i).unwrap_or(0);

                let bullish_crossover = p_fast <= p_slow && c_fast > c_slow;
                let bearish_crossover = p_fast >= p_slow && c_fast < c_slow;

                if bullish_crossover {
                    if in_position && position_side == "sell" {
                        // Exit short
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(), // Buy to cover
                            size_hint: "100%".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason: "TEMA Bullish Crossover (Exit Short)".to_string(),
                            timestamp_ms: ts,
                        });
                    }

                    // Enter long
                    let sl = c_close - (c_atr * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100%".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: None, // Let trend ride
                        reason: "TEMA Bullish Crossover".to_string(),
                        timestamp_ms: ts,
                    });

                    in_position = true;
                    position_side = "buy";
                } else if bearish_crossover {
                    if in_position && position_side == "buy" {
                        // Exit long
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "100%".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason: "TEMA Bearish Crossover (Exit Long)".to_string(),
                            timestamp_ms: ts,
                        });
                    }

                    // Enter short
                    let sl = c_close + (c_atr * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100%".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: None,
                        reason: "TEMA Bearish Crossover".to_string(),
                        timestamp_ms: ts,
                    });

                    in_position = true;
                    position_side = "sell";
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: TemaCrossoverConfig = serde_json::from_value(params)?;
        new_config.validate()?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> DataFrame {
        // Needs a relatively long warmup due to EMA and ATR calculation periods
        let mut close_vals = vec![100.0; 50]; // Flat phase

        // Add an uptrend
        for i in 50..70 {
            close_vals.push(100.0 + (i - 50) as f64 * 2.0);
        }
        // Add a downtrend
        for i in 70..90 {
            close_vals.push(140.0 - (i - 70) as f64 * 2.0);
        }

        let len = close_vals.len();
        df!(
            "open" => close_vals.clone(),
            "high" => close_vals.iter().map(|v| v + 5.0).collect::<Vec<_>>(),
            "low" => close_vals.iter().map(|v| v - 5.0).collect::<Vec<_>>(),
            "close" => close_vals,
            "volume" => vec![1000.0; len],
            "timestamp_unix_ms" => (0..len).map(|i| (i as i64) * 86400000).collect::<Vec<_>>()
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_tema_crossover_signals() {
        let data = create_test_data();
        let config = TemaCrossoverConfig {
            fast_period: 5,
            slow_period: 15,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = TemaCrossover::new(config);
        let signals = strategy.generate_signals(&data).await.unwrap();

        assert!(!signals.is_empty(), "Should generate signals");

        // Verify we have both entry and exit/reversal signals
        let buys: Vec<_> = signals
            .iter()
            .filter(|s| s.side == "buy" && s.signal_type == SignalType::Entry)
            .collect();
        let sells: Vec<_> = signals
            .iter()
            .filter(|s| s.side == "sell" && s.signal_type == SignalType::Entry)
            .collect();

        assert!(!buys.is_empty(), "Should have buy entry signals");
        assert!(!sells.is_empty(), "Should have sell entry signals");
    }

    #[tokio::test]
    async fn test_tema_crossover_validation() {
        let mut config = TemaCrossoverConfig::default();
        config.fast_period = 20;
        config.slow_period = 10; // fast >= slow

        let strategy = TemaCrossover::new(config);
        let data = create_test_data();

        let result = strategy.generate_signals(&data).await;
        assert!(result.is_err(), "Should fail validation");
        assert_eq!(
            result.unwrap_err().to_string(),
            "Fast period must be less than slow period"
        );
    }
}
