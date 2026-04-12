use crate::indicators::atr;
use crate::strategy::{Signal, SignalType, Strategy, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AtrBreakoutConfig {
    pub atr_period: usize,
    pub breakout_multiplier: f64,
    pub max_position_size: f64,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}

pub struct AtrBreakout {
    config: AtrBreakoutConfig,
}

impl AtrBreakout {
    pub fn new(config: AtrBreakoutConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for AtrBreakout {
    fn name(&self) -> &str {
        "ATR Breakout Strategy"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if self.config.atr_period == 0 || self.config.breakout_multiplier <= 0.0 {
            anyhow::bail!("Invalid configuration parameters for AtrBreakout");
        }

        if data.height() < self.config.atr_period + 1 {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        for i in self.config.atr_period..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i);
            let prev_price_opt = close_arr.get(i - 1);
            let atr_opt = atr_arr.get(i - 1); // Use previous bar's ATR for breakout threshold

            if let (Some(price), Some(prev_price), Some(atr_val)) =
                (price_opt, prev_price_opt, atr_opt)
            {
                let long_threshold = prev_price + (atr_val * self.config.breakout_multiplier);
                let short_threshold = prev_price - (atr_val * self.config.breakout_multiplier);

                let sl_dist = atr_val * self.config.stop_loss_atr_mult;
                let size_hint = format!("{:.4}", self.config.max_position_size);

                // Long Entry
                if price > long_threshold {
                    let stop_loss = price - sl_dist;
                    let take_profit = price + (sl_dist * 2.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: size_hint.clone(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: Some(take_profit),
                        reason: format!(
                            "ATR Long Breakout (Price: {:.2} > Threshold: {:.2})",
                            price, long_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Short Entry
                else if price < short_threshold {
                    let stop_loss = price + sl_dist;
                    let take_profit = price - (sl_dist * 2.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: size_hint.clone(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: Some(take_profit),
                        reason: format!(
                            "ATR Short Breakout (Price: {:.2} < Threshold: {:.2})",
                            price, short_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit - when price drops below the previous close minus ATR multiplier
                if price < prev_price - (atr_val * 1.0) {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("ATR Long Exit (Price dropped below prev close - ATR)"),
                        timestamp_ms: timestamp,
                    });
                }
                // Short Exit - when price rises above the previous close plus ATR multiplier
                else if price > prev_price + (atr_val * 1.0) {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("ATR Short Exit (Price rose above prev close + ATR)"),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: AtrBreakoutConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_entry_signal_generation() -> Result<()> {
        let config = AtrBreakoutConfig {
            atr_period: 2,
            breakout_multiplier: 1.5,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            symbol: "TEST".to_string(),
        };
        let strategy = AtrBreakout::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "open"  => &[100.0, 100.0, 100.0, 100.0, 100.0],
            "high"  => &[102.0, 102.0, 102.0, 130.0, 102.0],
            "low"   => &[ 98.0,  98.0,  98.0, 100.0,  98.0],
            "close" => &[100.0, 100.0, 100.0, 125.0, 100.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let entry_signals: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        assert!(!entry_signals.is_empty(), "Should generate an entry signal");

        let first_entry = &entry_signals[0];
        assert_eq!(first_entry.side, "buy");
        assert!(first_entry.stop_loss.is_some());
        assert_eq!(first_entry.size_hint, "100.0000");

        Ok(())
    }

    #[tokio::test]
    async fn test_exit_signal_generation() -> Result<()> {
        let config = AtrBreakoutConfig {
            atr_period: 2,
            breakout_multiplier: 1.5,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            symbol: "TEST".to_string(),
        };
        let strategy = AtrBreakout::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "open"  => &[100.0, 100.0, 100.0, 100.0, 100.0],
            "high"  => &[102.0, 102.0, 102.0, 102.0, 102.0],
            "low"   => &[ 98.0,  98.0,  98.0,  70.0,  98.0],
            "close" => &[100.0, 100.0, 100.0,  75.0, 100.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let exit_signals: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();
        assert!(!exit_signals.is_empty(), "Should generate an exit signal");

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let config = AtrBreakoutConfig {
            atr_period: 0,
            breakout_multiplier: 1.5,
            max_position_size: 100.0,
            stop_loss_atr_mult: 1.0,
            symbol: "TEST".to_string(),
        };
        let strategy = AtrBreakout::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000],
            "open"  => &[100.0, 100.0],
            "high"  => &[102.0, 102.0],
            "low"   => &[ 98.0,  98.0],
            "close" => &[100.0, 100.0]
        )?;

        let result = strategy.generate_signals(&df).await;
        assert!(result.is_err(), "Should fail parameter validation");

        Ok(())
    }
}
