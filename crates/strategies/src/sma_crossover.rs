use crate::indicators::{atr, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmaCrossoverConfig {
    pub short_window: usize,
    pub long_window: usize,
    pub stop_loss_pct: f64, // Keep for fallback, but prefer ATR
    pub atr_period: usize,
    pub atr_mult: f64,
    pub symbol: String,
}

impl StrategyConfig for SmaCrossoverConfig {}

pub struct SmaCrossover {
    config: SmaCrossoverConfig,
}

impl SmaCrossover {
    pub fn new(config: SmaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for SmaCrossover {
    fn name(&self) -> &str {
        "SmaCrossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.i64()?;

        // Calculate indicators
        let short_sma = sma::calculate(data, self.config.short_window)?;
        let short_sma_arr = short_sma.f64()?;

        let long_sma = sma::calculate(data, self.config.long_window)?;
        let long_sma_arr = long_sma.f64()?;

        // Calculate ATR for stop loss
        let atr = atr::calculate(data, self.config.atr_period).unwrap_or(Series::new_null(
            "atr".into(),
            data.height(),
        ));
        let atr_arr = if atr.null_count() == atr.len() {
            None
        } else {
            Some(atr.f64()?)
        };

        let mut signals = Vec::new();
        let mut in_position = false;

        for i in 1..data.height() {
            let current_short = short_sma_arr.get(i);
            let prev_short = short_sma_arr.get(i - 1);
            let current_long = long_sma_arr.get(i);
            let prev_long = long_sma_arr.get(i - 1);

            let close_price = close_arr.get(i);
            let timestamp = time_arr.get(i).unwrap_or(0);

            if current_short.is_none()
                || prev_short.is_none()
                || current_long.is_none()
                || prev_long.is_none()
                || close_price.is_none()
            {
                continue;
            }

            let cs = current_short.unwrap();
            let ps = prev_short.unwrap();
            let cl = current_long.unwrap();
            let pl = prev_long.unwrap();
            let price = close_price.unwrap();

            // Entry Condition: Short SMA crosses above Long SMA
            if cs > cl && ps <= pl && !in_position {
                in_position = true;

                // Determine stop loss
                let stop_loss = if let Some(arr) = &atr_arr {
                    if let Some(atr_val) = arr.get(i) {
                        Some(price - (atr_val * self.config.atr_mult))
                    } else {
                        Some(price * (1.0 - self.config.stop_loss_pct))
                    }
                } else {
                    Some(price * (1.0 - self.config.stop_loss_pct))
                };

                signals.push(Signal {
                    signal_type: SignalType::Entry,
                    symbol: self.config.symbol.clone(),
                    side: "buy".to_string(),
                    size_hint: "100".to_string(),
                    confidence: 0.8,
                    stop_loss,
                    take_profit: None,
                    reason: format!("SMA Golden Cross: Short ({:.2}) > Long ({:.2})", cs, cl),
                    timestamp_ms: timestamp,
                });
            }
            // Exit Condition: Short SMA crosses below Long SMA
            else if cs < cl && ps >= pl && in_position {
                in_position = false;
                signals.push(Signal {
                    signal_type: SignalType::Exit,
                    symbol: self.config.symbol.clone(),
                    side: "sell".to_string(),
                    size_hint: "max".to_string(),
                    confidence: 0.8,
                    stop_loss: None, // No stop loss on exit
                    take_profit: None,
                    reason: format!("SMA Death Cross: Short ({:.2}) < Long ({:.2})", cs, cl),
                    timestamp_ms: timestamp,
                });
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: SmaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_sma_entry_exit_stateless() -> Result<()> {
        let config = SmaCrossoverConfig {
            short_window: 2,
            long_window: 3,
            stop_loss_pct: 0.1,
            atr_period: 2,
            atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = SmaCrossover::new(config);

        // Construct data to force a crossover
        // short_window = 2, long_window = 3
        // Need short < long, then short > long (entry), then short < long (exit)
        let closes = vec![
            100.0, 100.0, 100.0, // Stable, sma2=100, sma3=100
            90.0,  90.0,  90.0,  // Drop, sma2=90, sma3=93.33 (Short < Long)
            110.0, 120.0, 120.0, // Rise, sma2=115, sma3=106.66 (Short > Long -> ENTRY)
            80.0,  80.0,  80.0,  // Drop, sma2=80, sma3=93.33 (Short < Long -> EXIT)
        ];
        let highs: Vec<f64> = closes.iter().map(|&c| c + 1.0).collect();
        let lows: Vec<f64> = closes.iter().map(|&c| c - 1.0).collect();
        let timestamps: Vec<i64> = (0..closes.len()).map(|i| i as i64 * 1000).collect();

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        assert!(!entries.is_empty(), "Should generate at least one entry signal");
        assert!(!exits.is_empty(), "Should generate at least one exit signal");

        Ok(())
    }

    #[tokio::test]
    async fn test_sma_parameter_update() -> Result<()> {
        let mut strategy = SmaCrossover::new(SmaCrossoverConfig {
            short_window: 2,
            long_window: 3,
            stop_loss_pct: 0.1,
            atr_period: 2,
            atr_mult: 2.0,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "short_window": 10,
            "long_window": 20,
            "stop_loss_pct": 0.05,
            "atr_period": 14,
            "atr_mult": 1.5,
            "symbol": "NEW"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.short_window, 10);
        assert_eq!(strategy.config.long_window, 20);
        assert_eq!(strategy.config.symbol, "NEW");
        Ok(())
    }

    #[tokio::test]
    async fn test_sma_empty_data() -> Result<()> {
        let config = SmaCrossoverConfig {
            short_window: 2,
            long_window: 3,
            stop_loss_pct: 0.1,
            atr_period: 2,
            atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = SmaCrossover::new(config);

        let df = df!(
            "timestamp_unix_ms" => Vec::<i64>::new(),
            "close" => Vec::<f64>::new(),
            "high" => Vec::<f64>::new(),
            "low" => Vec::<f64>::new()
        )?;

        let result = strategy.generate_signals(&df).await;
        assert!(result.is_err(), "Should handle empty data gracefully");

        Ok(())
    }
}
