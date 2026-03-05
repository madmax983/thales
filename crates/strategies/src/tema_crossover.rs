use crate::indicators::{atr, tema};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TemaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for TemaCrossoverConfig {}

pub struct TemaCrossover {
    config: TemaCrossoverConfig,
}

impl TemaCrossover {
    pub fn new(config: TemaCrossoverConfig) -> Self {
        Self { config }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_single_signal(
        &self,
        close: f64,
        short_tema: f64,
        long_tema: f64,
        prev_short_tema: f64,
        prev_long_tema: f64,
        atr_val: f64,
        timestamp_ms: i64,
    ) -> Option<Signal> {
        // Buy: short TEMA crosses above long TEMA
        let crossover_up = short_tema > long_tema && prev_short_tema <= prev_long_tema;
        // Sell: short TEMA crosses below long TEMA
        let crossover_down = short_tema < long_tema && prev_short_tema >= prev_long_tema;

        if crossover_up {
            let stop_loss = close - (atr_val * self.config.stop_loss_atr_mult);
            Some(Signal {
                signal_type: SignalType::Entry,
                symbol: self.config.symbol.clone(),
                side: "buy".to_string(),
                size_hint: "100%".to_string(),
                confidence: 0.8,
                stop_loss: Some(stop_loss),
                take_profit: None,
                reason: "TEMA Golden Cross".to_string(),
                timestamp_ms,
            })
        } else if crossover_down {
            Some(Signal {
                signal_type: SignalType::Exit,
                symbol: self.config.symbol.clone(),
                side: "sell".to_string(),
                size_hint: "100%".to_string(),
                confidence: 0.8,
                stop_loss: None,
                take_profit: None,
                reason: "TEMA Death Cross".to_string(),
                timestamp_ms,
            })
        } else {
            None
        }
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
        if data.height() < self.config.long_period + 1 {
            return Ok(vec![]);
        }

        let short_tema_series = tema::calculate(data, self.config.short_period)?;
        let long_tema_series = tema::calculate(data, self.config.long_period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let short_tema_ca = short_tema_series.f64()?;
        let long_tema_ca = long_tema_series.f64()?;
        let atr_ca = atr_series.f64()?;

        let close_series = data.column("close")?.f64()?;
        let ts_series = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let mut in_position = false;

        for i in 1..data.height() {
            let short_tema = short_tema_ca.get(i);
            let long_tema = long_tema_ca.get(i);
            let prev_short_tema = short_tema_ca.get(i - 1);
            let prev_long_tema = long_tema_ca.get(i - 1);

            let close = close_series.get(i);
            let ts = ts_series.get(i);
            let atr_val = atr_ca.get(i).unwrap_or(0.0);

            if let (
                Some(st),
                Some(lt),
                Some(pst),
                Some(plt),
                Some(c),
                Some(t),
            ) = (short_tema, long_tema, prev_short_tema, prev_long_tema, close, ts) {
                if let Some(signal) = self.generate_single_signal(c, st, lt, pst, plt, atr_val, t) {
                    if signal.signal_type == SignalType::Entry && !in_position {
                        signals.push(signal);
                        in_position = true;
                    } else if signal.signal_type == SignalType::Exit && in_position {
                        signals.push(signal);
                        in_position = false;
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: TemaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        // Needs a long enough series for TEMA. Wait for 20 points + TEMA warm up
        let mut close_prices = vec![100.0; 30]; // flat

        // Uptrend
        for i in 30..40 {
            close_prices.push(100.0 + (i - 30) as f64 * 2.0);
        }

        // Downtrend
        for i in 40..50 {
            close_prices.push(120.0 - (i - 40) as f64 * 3.0);
        }

        let timestamps: Vec<i64> = (0..close_prices.len()).map(|i| (i as i64) * 1000).collect();
        let highs = close_prices.iter().map(|c| c + 1.0).collect::<Vec<_>>();
        let lows = close_prices.iter().map(|c| c - 1.0).collect::<Vec<_>>();

        df!(
            "timestamp_unix_ms" => timestamps,
            "open" => close_prices.clone(),
            "high" => highs,
            "low" => lows,
            "close" => close_prices.clone(),
            "volume" => vec![1000.0; close_prices.len()]
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_tema_crossover_signals() -> Result<()> {
        let df = create_test_data();

        let config = TemaCrossoverConfig {
            short_period: 3,
            long_period: 8,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = TemaCrossover::new(config);
        let signals = strategy.generate_signals(&df).await?;

        // Should have an entry during uptrend and an exit during downtrend
        assert!(!signals.is_empty(), "Should generate signals");

        let mut has_entry = false;
        let mut has_exit = false;

        for sig in signals {
            if sig.signal_type == SignalType::Entry {
                has_entry = true;
                assert!(sig.stop_loss.is_some());
            }
            if sig.signal_type == SignalType::Exit {
                has_exit = true;
            }
        }

        assert!(has_entry, "Should generate entry signal");
        assert!(has_exit, "Should generate exit signal");

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let config = TemaCrossoverConfig {
            short_period: 3,
            long_period: 8,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = TemaCrossover::new(config);
        let df = DataFrame::default();
        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let config = TemaCrossoverConfig {
            short_period: 3,
            long_period: 8,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let mut strategy = TemaCrossover::new(config);

        let new_params = serde_json::json!({
            "short_period": 5,
            "long_period": 10,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 20,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.short_period, 5);
        assert_eq!(strategy.config.long_period, 10);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
