use crate::indicators::{atr, wma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

pub struct WmaCrossover {
    config: WmaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WmaCrossoverConfig {
    pub short_window: usize,
    pub long_window: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for WmaCrossoverConfig {}

impl WmaCrossover {
    pub fn new(config: WmaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for WmaCrossover {
    fn name(&self) -> &str {
        "WmaCrossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        let short_wma = wma::calculate(data, self.config.short_window)?;
        let long_wma = wma::calculate(data, self.config.long_window)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let short_wma_f64 = short_wma.f64()?;
        let long_wma_f64 = long_wma.f64()?;
        let atr_f64 = atr_series.f64()?;

        let close_series = data.column("close")?.f64()?;
        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let mut in_position = false;
        let mut current_stop_loss = 0.0;

        for i in 1..data.height() {
            let current_close = close_series.get(i);
            let current_short_wma = short_wma_f64.get(i);
            let current_long_wma = long_wma_f64.get(i);
            let prev_short_wma = short_wma_f64.get(i - 1);
            let prev_long_wma = long_wma_f64.get(i - 1);
            let current_atr = atr_f64.get(i);
            let timestamp = timestamps.get(i).unwrap_or(0);

            if let (
                Some(c),
                Some(curr_s_wma),
                Some(curr_l_wma),
                Some(prev_s_wma),
                Some(prev_l_wma),
                Some(atr),
            ) = (
                current_close,
                current_short_wma,
                current_long_wma,
                prev_short_wma,
                prev_long_wma,
                current_atr,
            ) {
                if !in_position {
                    // Check for Entry (Short crosses above Long WMA)
                    if prev_s_wma <= prev_l_wma && curr_s_wma > curr_l_wma {
                        in_position = true;
                        current_stop_loss = c - (atr * self.config.stop_loss_atr_mult);

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(current_stop_loss),
                            take_profit: None,
                            reason: "Short WMA crossed above Long WMA".to_string(),
                            timestamp_ms: timestamp,
                        });
                    }
                } else {
                    // Check for Exit (Stop loss hit or Short crosses below Long WMA)
                    let mut exit_reason = None;

                    if c <= current_stop_loss {
                        exit_reason = Some("Stop Loss Hit".to_string());
                    } else if prev_s_wma >= prev_l_wma && curr_s_wma < curr_l_wma {
                        exit_reason = Some("Short WMA crossed below Long WMA".to_string());
                    }

                    // Update stop loss trailing (if price moves in our favor)
                    let new_stop = c - (atr * self.config.stop_loss_atr_mult);
                    if new_stop > current_stop_loss {
                        current_stop_loss = new_stop;
                    }

                    if let Some(reason) = exit_reason {
                        in_position = false;
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason,
                            timestamp_ms: timestamp,
                        });
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: WmaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        // Needs enough data for WMA(20) and ATR(14)
        let mut close_vals = vec![100.0; 50];
        let mut high_vals = vec![101.0; 50];
        let mut low_vals = vec![99.0; 50];
        let timestamps: Vec<i64> = (0..50).map(|i| i as i64 * 1000).collect();

        // Create a trend down to establish Short WMA < Long WMA
        for i in 10..30 {
            close_vals[i] = 100.0 - (i as f64 - 10.0);
            high_vals[i] = close_vals[i] + 1.0;
            low_vals[i] = close_vals[i] - 1.0;
        }
        // Create a trend up to force Short WMA to cross above Long WMA
        for i in 30..50 {
            close_vals[i] = 80.0 + (i as f64 - 30.0) * 3.0;
            high_vals[i] = close_vals[i] + 1.0;
            low_vals[i] = close_vals[i] - 1.0;
        }

        df!(
            "timestamp_unix_ms" => timestamps,
            "open" => close_vals.clone(),
            "high" => high_vals,
            "low" => low_vals,
            "close" => close_vals,
            "volume" => vec![1000.0; 50]
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_wma_crossover_empty_data() -> Result<()> {
        let config = WmaCrossoverConfig {
            short_window: 9,
            long_window: 21,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = WmaCrossover::new(config);
        let empty_df = DataFrame::empty();
        let signals = strategy.generate_signals(&empty_df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_wma_crossover_signal_generation() -> Result<()> {
        let config = WmaCrossoverConfig {
            short_window: 5,
            long_window: 10,
            stop_loss_atr_mult: 1.0,
            atr_period: 5,
            symbol: "TEST".to_string(),
        };

        let strategy = WmaCrossover::new(config);
        let df = create_test_data();
        let signals = strategy.generate_signals(&df).await?;

        // We expect at least one entry signal because of the designed V-shape recovery
        assert!(!signals.is_empty(), "Should generate signals");

        let entry_signal = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry_signal.is_some(), "Should have an entry signal");
        if let Some(s) = entry_signal {
            assert_eq!(s.side, "buy");
            assert!(s.stop_loss.is_some());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_wma_crossover_parameter_validation() -> Result<()> {
        let mut strategy = WmaCrossover::new(WmaCrossoverConfig {
            short_window: 9,
            long_window: 21,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "short_window": 5,
            "long_window": 10,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 5,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.short_window, 5);
        assert_eq!(strategy.config.long_window, 10);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
