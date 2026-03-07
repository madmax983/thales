use crate::indicators::{atr, tema};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

/// Configuration for the TEMA Crossover strategy.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TemaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for TemaCrossoverConfig {}

impl Default for TemaCrossoverConfig {
    fn default() -> Self {
        Self {
            short_period: 9,
            long_period: 21,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "BTCUSD".to_string(),
        }
    }
}

/// TEMA Crossover Strategy
///
/// A trend-following strategy that generates signals based on the crossover of two
/// Triple Exponential Moving Averages (TEMA) of different periods.
pub struct TemaCrossover {
    config: TemaCrossoverConfig,
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
        if data.height() < self.config.long_period * 3 {
            return Ok(vec![]);
        }

        let short_tema_series = tema::calculate(data, self.config.short_period)?;
        let long_tema_series = tema::calculate(data, self.config.long_period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let short_tema_f64 = short_tema_series.f64()?;
        let long_tema_f64 = long_tema_series.f64()?;
        let atr_f64 = atr_series.f64()?;

        let close_col = data.column("close")?.f64()?;
        let time_col = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let mut in_long = false;
        let mut _entry_price = 0.0;
        let mut stop_loss = 0.0;

        for i in 1..data.height() {
            let curr_short = short_tema_f64.get(i);
            let prev_short = short_tema_f64.get(i - 1);
            let curr_long = long_tema_f64.get(i);
            let prev_long = long_tema_f64.get(i - 1);

            let close = close_col.get(i);
            let atr_val = atr_f64.get(i);
            let timestamp = time_col.get(i).unwrap_or(0);

            if let (
                Some(c_short),
                Some(p_short),
                Some(c_long),
                Some(p_long),
                Some(price),
                Some(atr),
            ) = (
                curr_short,
                prev_short,
                curr_long,
                prev_long,
                close,
                atr_val,
            ) {
                if in_long && price <= stop_loss {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 1.0,
                        stop_loss: None,
                        take_profit: None,
                        reason: "TEMA Crossover Stop Loss Hit".to_string(),
                        timestamp_ms: timestamp,
                    });
                    in_long = false;
                    continue;
                }

                if !in_long && p_short <= p_long && c_short > c_long {
                    let calculated_sl = price - (atr * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(calculated_sl),
                        take_profit: None,
                        reason: "Short TEMA crossed above Long TEMA".to_string(),
                        timestamp_ms: timestamp,
                    });
                    in_long = true;
                    _entry_price = price;
                    stop_loss = calculated_sl;
                } else if in_long && p_short >= p_long && c_short < c_long {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Short TEMA crossed below Long TEMA".to_string(),
                        timestamp_ms: timestamp,
                    });
                    in_long = false;
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

    fn create_mock_data(size: usize) -> DataFrame {
        let close: Vec<f64> = (0..size)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let high: Vec<f64> = close.iter().map(|c| c + 1.0).collect();
        let low: Vec<f64> = close.iter().map(|c| c - 1.0).collect();
        let open: Vec<f64> = close.clone();
        let volume: Vec<f64> = vec![1000.0; size];
        let timestamp: Vec<i64> = (0..size).map(|i| i as i64 * 60000).collect();

        df!(
            "open" => open,
            "high" => high,
            "low" => low,
            "close" => close,
            "volume" => volume,
            "timestamp_unix_ms" => timestamp
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let strategy = TemaCrossover::new(TemaCrossoverConfig::default());
        let df = DataFrame::default();
        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_signal_generation() -> Result<()> {
        let strategy = TemaCrossover::new(TemaCrossoverConfig {
            short_period: 3,
            long_period: 6,
            stop_loss_atr_mult: 2.0,
            atr_period: 5,
            symbol: "TEST".to_string(),
        });

        let df = create_mock_data(100);
        let signals = strategy.generate_signals(&df).await?;

        assert!(!signals.is_empty(), "Should generate some signals");

        let mut in_position = false;
        for sig in signals {
            if sig.signal_type == SignalType::Entry {
                assert!(!in_position);
                assert_eq!(sig.side, "buy");
                assert!(sig.stop_loss.is_some());
                in_position = true;
            } else if sig.signal_type == SignalType::Exit {
                assert!(in_position);
                assert_eq!(sig.side, "sell");
                in_position = false;
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let mut strategy = TemaCrossover::new(TemaCrossoverConfig::default());
        let new_params = serde_json::json!({
            "short_period": 5,
            "long_period": 10,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 10,
            "symbol": "ETHUSD"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.short_period, 5);
        assert_eq!(strategy.config.long_period, 10);
        assert_eq!(strategy.config.symbol, "ETHUSD");

        Ok(())
    }
}
