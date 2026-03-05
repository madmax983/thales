use crate::indicators::{atr, tema};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemaCrossoverConfig {
    pub fast_period: usize,
    pub slow_period: usize,
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
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate Fast TEMA
        let fast_tema_series = tema::calculate(data, self.config.fast_period)?;
        let fast_tema_arr = fast_tema_series.f64()?;

        // Calculate Slow TEMA
        let slow_tema_series = tema::calculate(data, self.config.slow_period)?;
        let slow_tema_arr = slow_tema_series.f64()?;

        // Calculate ATR for stop loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let len = data.height();

        if len < 2 {
            return Ok(signals);
        }

        if self.config.fast_period >= self.config.slow_period {
            anyhow::bail!("fast_period must be less than slow_period");
        }

        let mut position: Option<String> = None;

        for i in 1..len {
            let current_close = close_arr.get(i);
            let ts = time_arr.get(i);

            let fast_curr = fast_tema_arr.get(i);
            let fast_prev = fast_tema_arr.get(i - 1);

            let slow_curr = slow_tema_arr.get(i);
            let slow_prev = slow_tema_arr.get(i - 1);

            let current_atr = atr_arr.get(i);

            if let (
                Some(close),
                Some(ts),
                Some(fc),
                Some(fp),
                Some(sc),
                Some(sp),
                Some(atr)
            ) = (
                current_close,
                ts,
                fast_curr,
                fast_prev,
                slow_curr,
                slow_prev,
                current_atr
            ) {
                // Bullish crossover: Fast TEMA crosses above Slow TEMA
                let crosses_above = fp <= sp && fc > sc;
                // Bearish crossover: Fast TEMA crosses below Slow TEMA
                let crosses_below = fp >= sp && fc < sc;

                if position.is_none() && crosses_above {
                    // Enter Long
                    let stop_loss = close - (atr * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: "Fast TEMA crossed above Slow TEMA".to_string(),
                        timestamp_ms: ts,
                    });
                    position = Some("long".to_string());
                } else if position.as_deref() == Some("long") && crosses_below {
                    // Exit Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Fast TEMA crossed below Slow TEMA".to_string(),
                        timestamp_ms: ts,
                    });
                    position = None;
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, _params: serde_json::Value) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        // Need enough data points to satisfy TEMA's triple EMA warmup
        // Fast TEMA (10) warmup is ~30. Slow TEMA (30) warmup is ~90.
        let len = 200;
        let closes: Vec<f64> = (0..len)
            .map(|i| {
                if i < 150 {
                    100.0 + (i as f64)
                } else {
                    // Make the drop steeper so it surely crosses below
                    250.0 - ((i - 150) as f64) * 5.0
                }
            })
            .collect();

        let highs: Vec<f64> = closes.iter().map(|&c| c + 2.0).collect();
        let lows: Vec<f64> = closes.iter().map(|&c| c - 2.0).collect();
        let timestamps: Vec<i64> = (0..len).map(|i| 1622505600000 + (i as i64 * 86400000)).collect();

        df!(
            "close" => &closes,
            "high" => &highs,
            "low" => &lows,
            "timestamp_unix_ms" => &timestamps
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_tema_crossover_signals() {
        let df = create_test_data();
        let config = TemaCrossoverConfig {
            fast_period: 5,
            slow_period: 15,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = TemaCrossover::new(config);
        let signals = strategy.generate_signals(&df).await.unwrap();

        assert!(!signals.is_empty(), "Should generate signals on clear trend reversal");

        // The first signal should be an entry (buy) as the trend goes up initially
        let first_signal = signals.first().unwrap();
        assert_eq!(first_signal.signal_type, SignalType::Entry);
        assert_eq!(first_signal.side, "buy");
        assert_eq!(first_signal.size_hint, "100");
        assert!(first_signal.stop_loss.is_some());

        // There should be a later signal to exit (sell) as the trend reverses downwards
        let has_exit = signals.iter().any(|s| s.signal_type == SignalType::Exit && s.side == "sell");
        assert!(has_exit, "Should generate an exit signal when trend reverses");
    }

    #[tokio::test]
    async fn test_empty_data() {
        let df = DataFrame::default();
        let config = TemaCrossoverConfig {
            fast_period: 5,
            slow_period: 15,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = TemaCrossover::new(config);
        let res = strategy.generate_signals(&df).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let df = create_test_data();
        let config = TemaCrossoverConfig {
            fast_period: 15, // fast >= slow
            slow_period: 15,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = TemaCrossover::new(config);
        let res = strategy.generate_signals(&df).await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "fast_period must be less than slow_period");
    }
}
