use crate::indicators::chandelier_exit;
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

pub struct ChandelierExit {
    config: ChandelierExitConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChandelierExitConfig {
    pub period: usize,
    pub atr_period: usize,
    pub multiplier: f64,
    pub symbol: String,
}

impl StrategyConfig for ChandelierExitConfig {}

impl ChandelierExit {
    pub fn new(config: ChandelierExitConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for ChandelierExit {
    fn name(&self) -> &str {
        "ChandelierExit"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?;
        let close = close_series.f64()?;
        let timestamp = data.column("timestamp_unix_ms")?.i64()?;

        // Calculate Chandelier Exit
        let (long_exit_series, short_exit_series) =
            chandelier_exit::calculate(data, self.config.period, self.config.multiplier)?;
        let long_exit = long_exit_series.f64()?;
        let short_exit = short_exit_series.f64()?;

        let mut signals = Vec::new();

        for i in 1..close.len() {
            let prev_close = close.get(i - 1);
            let curr_close = close.get(i);

            let prev_long_exit = long_exit.get(i - 1);
            let curr_long_exit = long_exit.get(i);

            let prev_short_exit = short_exit.get(i - 1);
            let curr_short_exit = short_exit.get(i);

            let ts = timestamp.get(i);

            if let (
                Some(p_close),
                Some(c_close),
                Some(p_long),
                Some(c_long),
                Some(p_short),
                Some(c_short),
                Some(time_ms),
            ) = (
                prev_close,
                curr_close,
                prev_long_exit,
                curr_long_exit,
                prev_short_exit,
                curr_short_exit,
                ts,
            ) {
                // Long Entry: Close crosses above Long Exit
                if p_close <= p_long && c_close > c_long {
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(c_long), // Stop loss is the chandelier line itself
                        take_profit: None,
                        reason: "Close crossed above Long Chandelier Exit".to_string(),
                        timestamp_ms: time_ms,
                    });
                }
                // Short Entry: Close crosses below Short Exit
                else if p_close >= p_short && c_close < c_short {
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(c_short), // Stop loss is the chandelier line itself
                        take_profit: None,
                        reason: "Close crossed below Short Chandelier Exit".to_string(),
                        timestamp_ms: time_ms,
                    });
                }

                // Long Exit: Close crosses below Long Exit
                if p_close >= p_long && c_close < c_long {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Close crossed below Long Chandelier Exit (Exit Long)".to_string(),
                        timestamp_ms: time_ms,
                    });
                }
                // Short Exit: Close crosses above Short Exit
                else if p_close <= p_short && c_close > c_short {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Close crossed above Short Chandelier Exit (Exit Short)"
                            .to_string(),
                        timestamp_ms: time_ms,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: ChandelierExitConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000],
            "high" => &[10.0, 11.0, 12.0, 11.5, 12.5, 13.0, 10.0, 9.0],
            "low" => &[9.0, 10.0, 10.5, 10.0, 11.0, 11.5, 8.0, 7.0],
            "close" => &[9.5, 10.5, 11.0, 10.5, 11.5, 12.0, 8.5, 7.5],
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_chandelier_exit_signals() -> Result<()> {
        let data = create_test_data();

        let config = ChandelierExitConfig {
            period: 3,
            atr_period: 3,
            multiplier: 1.0,
            symbol: "TEST".to_string(),
        };

        let strategy = ChandelierExit::new(config);
        let signals = strategy.generate_signals(&data).await?;

        // We expect at least some signals to be generated
        assert!(!signals.is_empty(), "Should generate signals");

        // Look for long entry
        let long_entry = signals
            .iter()
            .find(|s| s.side == "buy" && s.signal_type == SignalType::Entry);

        // Look for long exit or short entry
        let long_exit = signals
            .iter()
            .find(|s| s.side == "sell" && s.signal_type == SignalType::Exit);

        let short_entry = signals
            .iter()
            .find(|s| s.side == "sell" && s.signal_type == SignalType::Entry);

        assert!(long_entry.is_some() || short_entry.is_some() || long_exit.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let data = DataFrame::default();
        let config = ChandelierExitConfig {
            period: 14,
            atr_period: 14,
            multiplier: 3.0,
            symbol: "TEST".to_string(),
        };

        let strategy = ChandelierExit::new(config);
        let signals = strategy.generate_signals(&data).await?;

        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = ChandelierExit::new(ChandelierExitConfig {
            period: 14,
            atr_period: 14,
            multiplier: 3.0,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "period": 22,
            "atr_period": 22,
            "multiplier": 2.0,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.period, 22);
        assert_eq!(strategy.config.atr_period, 22);
        assert!((strategy.config.multiplier - 2.0).abs() < f64::EPSILON);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
