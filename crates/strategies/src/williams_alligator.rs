use crate::indicators::sma;
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WilliamsAlligatorConfig {
    pub jaw_period: usize,
    pub jaw_shift: usize,
    pub teeth_period: usize,
    pub teeth_shift: usize,
    pub lips_period: usize,
    pub lips_shift: usize,
    pub stop_loss_pct: f64,
    pub max_position_size: f64,
    pub symbol: String,
}

impl StrategyConfig for WilliamsAlligatorConfig {}

pub struct WilliamsAlligator {
    config: WilliamsAlligatorConfig,
}

impl WilliamsAlligator {
    pub fn new(config: WilliamsAlligatorConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for WilliamsAlligator {
    fn name(&self) -> &str {
        "WilliamsAlligator"
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

        // Calculate SMAs
        let jaw_sma = sma::calculate(data, self.config.jaw_period)?;
        let teeth_sma = sma::calculate(data, self.config.teeth_period)?;
        let lips_sma = sma::calculate(data, self.config.lips_period)?;

        let jaw_arr = jaw_sma.f64()?;
        let teeth_arr = teeth_sma.f64()?;
        let lips_arr = lips_sma.f64()?;

        let mut signals = Vec::new();

        for i in self.config.jaw_period + self.config.jaw_shift..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let jaw_curr = jaw_arr.get(i - self.config.jaw_shift).unwrap_or(0.0);
            let teeth_curr = teeth_arr.get(i - self.config.teeth_shift).unwrap_or(0.0);
            let lips_curr = lips_arr.get(i - self.config.lips_shift).unwrap_or(0.0);

            let jaw_prev = jaw_arr.get(i - self.config.jaw_shift - 1).unwrap_or(0.0);
            let teeth_prev = teeth_arr
                .get(i - self.config.teeth_shift - 1)
                .unwrap_or(0.0);
            let lips_prev = lips_arr.get(i - self.config.lips_shift - 1).unwrap_or(0.0);

            let price = close_arr.get(i).unwrap_or(0.0);

            if lips_curr > teeth_curr
                && teeth_curr > jaw_curr
                && (lips_prev <= teeth_prev || teeth_prev <= jaw_prev)
            {
                let sl = price * (1.0 - self.config.stop_loss_pct);
                signals.push(Signal {
                    signal_type: SignalType::Entry,
                    symbol: self.config.symbol.clone(),
                    side: "buy".to_string(),
                    size_hint: self.config.max_position_size.to_string(),
                    confidence: 0.8,
                    stop_loss: Some(sl),
                    take_profit: None,
                    reason: "Alligator woke up bullish".to_string(),
                    timestamp_ms: timestamp,
                });
            } else if lips_curr < teeth_curr
                && teeth_curr < jaw_curr
                && (lips_prev >= teeth_prev || teeth_prev >= jaw_prev)
            {
                signals.push(Signal {
                    signal_type: SignalType::Entry,
                    symbol: self.config.symbol.clone(),
                    side: "sell".to_string(),
                    size_hint: self.config.max_position_size.to_string(),
                    confidence: 0.8,
                    stop_loss: Some(price * (1.0 + self.config.stop_loss_pct)),
                    take_profit: None,
                    reason: "Alligator woke up bearish".to_string(),
                    timestamp_ms: timestamp,
                });
            } else if lips_curr < teeth_curr && lips_prev >= teeth_prev {
                signals.push(Signal {
                    signal_type: SignalType::Exit,
                    symbol: self.config.symbol.clone(),
                    side: "sell".to_string(),
                    size_hint: "max".to_string(),
                    confidence: 0.8,
                    stop_loss: None,
                    take_profit: None,
                    reason: "Alligator sleeping (Exit Long)".to_string(),
                    timestamp_ms: timestamp,
                });
            } else if lips_curr > teeth_curr && lips_prev <= teeth_prev {
                signals.push(Signal {
                    signal_type: SignalType::Exit,
                    symbol: self.config.symbol.clone(),
                    side: "buy".to_string(),
                    size_hint: "max".to_string(),
                    confidence: 0.8,
                    stop_loss: None,
                    take_profit: None,
                    reason: "Alligator sleeping (Exit Short)".to_string(),
                    timestamp_ms: timestamp,
                });
            }
        }
        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: WilliamsAlligatorConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_williams_alligator_signals() -> Result<()> {
        let config = WilliamsAlligatorConfig {
            jaw_period: 13,
            jaw_shift: 8,
            teeth_period: 8,
            teeth_shift: 5,
            lips_period: 5,
            lips_shift: 3,
            stop_loss_pct: 0.05,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        };
        let strategy = WilliamsAlligator::new(config);

        let mut closes = Vec::new();
        let mut times = Vec::new();
        for i in 0..100 {
            let val = 100.0 + (i as f64 * 0.2).sin() * 10.0;
            closes.push(val);
            times.push(i as i64 * 1000);
        }

        let df = df!(
            "timestamp_unix_ms" => times,
            "close" => closes
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

        assert!(!entries.is_empty());
        assert!(!exits.is_empty());

        Ok(())
    }
}
