use crate::indicators::{atr, roc};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

pub struct RocMomentum {
    config: RocMomentumConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RocMomentumConfig {
    pub period: usize,
    pub buy_threshold: f64,
    pub sell_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for RocMomentumConfig {}

impl RocMomentum {
    pub fn new(config: RocMomentumConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for RocMomentum {
    fn name(&self) -> &str {
        "RocMomentum"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?;
        let close = close_series.f64()?;
        let timestamp = data.column("timestamp_unix_ms")?.i64()?;

        // Need enough data
        if close.len() <= self.config.period.max(self.config.atr_period) {
            return Ok(vec![]);
        }

        // Calculate indicators
        let roc_series = roc::calculate(data, self.config.period)?;
        let roc = roc_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr = atr_series.f64()?;

        let mut signals = Vec::new();

        // Loop through the data to find crossovers
        // A crossover is determined by looking at the current and previous candles
        for i in 1..close.len() {
            let prev_roc = roc.get(i - 1);
            let curr_roc = roc.get(i);

            let curr_close = close.get(i);
            let curr_atr = atr.get(i);
            let ts = timestamp.get(i);

            if let (Some(prev_r), Some(curr_r), Some(price), Some(atr_val), Some(time_ms)) =
                (prev_roc, curr_roc, curr_close, curr_atr, ts)
            {
                // Buy condition: ROC crosses above buy_threshold
                if prev_r <= self.config.buy_threshold && curr_r > self.config.buy_threshold {
                    let stop_loss = price - (atr_val * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(), // Fixed sizing as per other strategies or max
                        confidence: 0.8, // Arbitrary high confidence for a valid signal
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: "ROC crossed above buy threshold".to_string(),
                        timestamp_ms: time_ms,
                    });
                }
                // Sell condition: ROC crosses below sell_threshold
                else if prev_r >= self.config.sell_threshold
                    && curr_r < self.config.sell_threshold
                {
                    let stop_loss = price + (atr_val * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: "ROC crossed below sell threshold".to_string(),
                        timestamp_ms: time_ms,
                    });
                }

                // Exit conditions: crossing zero in the opposite direction
                // Exit Long
                if prev_r >= 0.0 && curr_r < 0.0 {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "ROC crossed below 0 (Exit Long)".to_string(),
                        timestamp_ms: time_ms,
                    });
                }
                // Exit Short
                else if prev_r <= 0.0 && curr_r > 0.0 {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "ROC crossed above 0 (Exit Short)".to_string(),
                        timestamp_ms: time_ms,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: RocMomentumConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        // Need high, low, close for ATR, close for ROC
        // Period = 3
        // Data len: 10
        df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000],
            "high" => &[10.0, 11.0, 12.0, 15.0, 14.0, 13.0, 10.0, 9.0, 8.0, 7.0],
            "low" => &[9.0, 10.0, 11.0, 11.0, 13.0, 12.0, 9.0, 8.0, 7.0, 6.0],
            "close" => &[10.0, 11.0, 12.0, 14.0, 13.0, 12.0, 10.0, 8.5, 7.5, 6.5],
        ).unwrap()
    }

    #[tokio::test]
    async fn test_roc_momentum_signals() -> Result<()> {
        let data = create_test_data();

        let config = RocMomentumConfig {
            period: 3,
            buy_threshold: 15.0,   // ROC needs to go > 15.0
            sell_threshold: -10.0, // ROC needs to go < -10.0
            stop_loss_atr_mult: 2.0,
            atr_period: 3,
            symbol: "TEST".to_string(),
        };

        let strategy = RocMomentum::new(config);
        let signals = strategy.generate_signals(&data).await?;

        // With period 3:
        // ROC at i=3 (14.0 vs 10.0) = +40% (prev is None/0, wait prev ROC is at i=2 vs None)
        // Let's trace:
        // i=0: c=10.0
        // i=1: c=11.0
        // i=2: c=12.0
        // i=3: c=14.0, prev=10.0 -> ROC = 40.0%
        // i=4: c=13.0, prev=11.0 -> ROC = 18.18%
        // i=5: c=12.0, prev=12.0 -> ROC = 0.0%
        // i=6: c=10.0, prev=14.0 -> ROC = -28.57%

        // Wait, prev_r is ROC at i-1.
        // At i=3: curr_roc = 40.0. prev_roc = None. No crossover.
        // At i=4: curr_roc = 18.18, prev_roc = 40.0.
        // Let's check when it crosses -10.0
        // At i=5: curr_roc = 0.0, prev_roc = 18.18
        // At i=6: curr_roc = -28.57, prev_roc = 0.0. Crosses below -10.0! Sell signal.

        assert!(!signals.is_empty(), "Should generate signals");

        // Find a sell signal
        let sell_sig = signals
            .iter()
            .find(|s| s.side == "sell" && s.signal_type == SignalType::Entry);
        assert!(sell_sig.is_some(), "Expected a sell signal");

        if let Some(sig) = sell_sig {
            assert_eq!(sig.reason, "ROC crossed below sell threshold");
        }

        // Test Buy Signal path
        let data2 = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000],
            "high" => &[10.0, 9.0, 8.0, 7.0, 11.0, 15.0],
            "low" => &[9.0, 8.0, 7.0, 6.0, 10.0, 14.0],
            "close" => &[10.0, 8.5, 7.5, 6.5, 10.5, 14.5], // ROC at i=4 (vs 8.5) = +23%, ROC at i=5 (vs 7.5) = +93%
            "volume" => &[100.0, 100.0, 100.0, 100.0, 100.0, 100.0],
        )
        .unwrap();

        let signals2 = strategy.generate_signals(&data2).await?;

        let buy_entry = signals2
            .iter()
            .find(|s| s.side == "buy" && s.signal_type == SignalType::Entry);
        assert!(buy_entry.is_some(), "Expected a buy entry signal");

        if let Some(sig) = buy_entry {
            assert_eq!(sig.reason, "ROC crossed above buy threshold");
        }

        // Test Exit Signal
        let exit_sig = signals.iter().find(|s| s.signal_type == SignalType::Exit);
        assert!(exit_sig.is_some(), "Expected an exit signal");

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let data = DataFrame::default();
        let config = RocMomentumConfig {
            period: 14,
            buy_threshold: 0.0,
            sell_threshold: 0.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = RocMomentum::new(config);
        let signals = strategy.generate_signals(&data).await?;

        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = RocMomentum::new(RocMomentumConfig {
            period: 14,
            buy_threshold: 0.0,
            sell_threshold: 0.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "period": 20,
            "buy_threshold": 5.0,
            "sell_threshold": -5.0,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 20,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.period, 20);
        assert_eq!(strategy.config.buy_threshold, 5.0);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
