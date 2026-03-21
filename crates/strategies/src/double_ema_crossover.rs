use crate::indicators::{atr, dema};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

pub struct DoubleEmaCrossover {
    config: DoubleEmaCrossoverConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoubleEmaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for DoubleEmaCrossoverConfig {
    fn default() -> Self {
        Self {
            short_period: 9,
            long_period: 21,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for DoubleEmaCrossoverConfig {}

impl DoubleEmaCrossover {
    pub fn new(config: DoubleEmaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for DoubleEmaCrossover {
    fn name(&self) -> &str {
        "DoubleEmaCrossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?;
        let time_series = data.column("timestamp_unix_ms")?.i64()?;

        let short_dema = dema::calculate(data, self.config.short_period)?;
        let long_dema = dema::calculate(data, self.config.long_period)?;
        let atr = atr::calculate(data, self.config.atr_period)?;

        let short_v: Vec<Option<f64>> = short_dema.f64()?.into_iter().collect();
        let long_v: Vec<Option<f64>> = long_dema.f64()?.into_iter().collect();
        let atr_v: Vec<Option<f64>> = atr.f64()?.into_iter().collect();
        let close_v: Vec<Option<f64>> = close_series.f64()?.into_iter().collect();

        let mut signals = Vec::new();
        let mut in_long = false;
        let mut in_short = false;
        let mut entry_price = 0.0;

        for i in 1..close_v.len() {
            if let (
                Some(short_current),
                Some(long_current),
                Some(short_prev),
                Some(long_prev),
                Some(close),
                Some(atr_val),
            ) = (
                short_v[i],
                long_v[i],
                short_v[i - 1],
                long_v[i - 1],
                close_v[i],
                atr_v[i],
            ) {
                let timestamp = time_series.get(i).unwrap_or(0);

                let cross_above = short_current > long_current && short_prev <= long_prev;
                let cross_below = short_current < long_current && short_prev >= long_prev;

                // Stop loss check
                if in_long {
                    let sl_price = entry_price - (atr_val * self.config.stop_loss_atr_mult);
                    if close <= sl_price {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason: "Stop Loss Hit".to_string(),
                            timestamp_ms: timestamp,
                        });
                        in_long = false;
                    }
                } else if in_short {
                    let sl_price = entry_price + (atr_val * self.config.stop_loss_atr_mult);
                    if close >= sl_price {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason: "Stop Loss Hit".to_string(),
                            timestamp_ms: timestamp,
                        });
                        in_short = false;
                    }
                }

                // Entry / Reverse Crossover
                if cross_above {
                    if in_short {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            stop_loss: None,
                            take_profit: None,
                            reason: "Short Exit: DEMA Crossover Up".to_string(),
                            timestamp_ms: timestamp,
                        });
                        in_short = false;
                    }
                    if !in_long {
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(close - (atr_val * self.config.stop_loss_atr_mult)),
                            take_profit: None,
                            reason: "Long Entry: DEMA Crossover Up".to_string(),
                            timestamp_ms: timestamp,
                        });
                        in_long = true;
                        entry_price = close;
                    }
                } else if cross_below {
                    if in_long {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            stop_loss: None,
                            take_profit: None,
                            reason: "Long Exit: DEMA Crossover Down".to_string(),
                            timestamp_ms: timestamp,
                        });
                        in_long = false;
                    }
                    if !in_short {
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(close + (atr_val * self.config.stop_loss_atr_mult)),
                            take_profit: None,
                            reason: "Short Entry: DEMA Crossover Down".to_string(),
                            timestamp_ms: timestamp,
                        });
                        in_short = true;
                        entry_price = close;
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: DoubleEmaCrossoverConfig = serde_json::from_value(params)?;
        anyhow::ensure!(
            new_config.short_period > 0,
            "short_period must be greater than 0"
        );
        anyhow::ensure!(
            new_config.long_period > 0,
            "long_period must be greater than 0"
        );
        anyhow::ensure!(
            new_config.short_period < new_config.long_period,
            "short_period must be less than long_period"
        );
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> DataFrame {
        // Generate enough data for EMA warmup (need > long_period + atr_period)
        let n = 100;
        let mut closes = Vec::with_capacity(n);
        let mut highs = Vec::with_capacity(n);
        let mut lows = Vec::with_capacity(n);
        let mut timestamps = Vec::with_capacity(n);

        let mut price = 100.0;
        // First 50 bars go up
        for i in 0..50 {
            closes.push(price);
            highs.push(price + 2.0);
            lows.push(price - 2.0);
            timestamps.push(i as i64 * 60000);
            price += 1.0;
        }

        // Next 50 bars go down (should trigger crossover)
        for i in 50..100 {
            closes.push(price);
            highs.push(price + 2.0);
            lows.push(price - 2.0);
            timestamps.push(i as i64 * 60000);
            price -= 2.0;
        }

        df!(
            "close" => closes,
            "high" => highs,
            "low" => lows,
            "timestamp_unix_ms" => timestamps,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_entry_and_exit_signals() {
        let df = create_test_data();
        let config = DoubleEmaCrossoverConfig {
            short_period: 5,
            long_period: 10,
            atr_period: 5,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };

        let strategy = DoubleEmaCrossover::new(config);
        let signals = strategy.generate_signals(&df).await.unwrap();

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();

        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        assert!(!entries.is_empty(), "Should generate entry signals");
        assert!(!exits.is_empty(), "Should generate exit signals");

        let first_entry = entries.first().unwrap();
        assert!(
            first_entry.stop_loss.is_some(),
            "Entry should have stop loss"
        );
        assert_eq!(first_entry.symbol, "TEST");
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let mut strategy = DoubleEmaCrossover::new(DoubleEmaCrossoverConfig::default());

        let invalid_params = serde_json::json!({
            "short_period": 20,
            "long_period": 10,
            "atr_period": 14,
            "stop_loss_atr_mult": 2.0,
            "symbol": "TEST"
        });

        let res = strategy.update_params(invalid_params).await;
        assert!(res.is_err(), "Should fail when short_period >= long_period");

        let invalid_params_zero = serde_json::json!({
            "short_period": 0,
            "long_period": 10,
            "atr_period": 14,
            "stop_loss_atr_mult": 2.0,
            "symbol": "TEST"
        });

        let res = strategy.update_params(invalid_params_zero).await;
        assert!(res.is_err(), "Should fail when short_period is 0");
    }
}
