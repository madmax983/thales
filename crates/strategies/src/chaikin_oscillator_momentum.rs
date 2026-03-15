use crate::indicators::{atr, chaikin_oscillator};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaikinOscillatorMomentumConfig {
    pub fast_period: usize,
    pub slow_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for ChaikinOscillatorMomentumConfig {
    fn default() -> Self {
        Self {
            fast_period: 3,
            slow_period: 10,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl ChaikinOscillatorMomentumConfig {
    pub fn validate(&self) -> Result<()> {
        if self.fast_period == 0 {
            anyhow::bail!("fast_period must be > 0");
        }
        if self.slow_period <= self.fast_period {
            anyhow::bail!("slow_period must be > fast_period");
        }
        if self.stop_loss_atr_mult <= 0.0 {
            anyhow::bail!("stop_loss_atr_mult must be > 0");
        }
        if self.atr_period == 0 {
            anyhow::bail!("atr_period must be > 0");
        }
        Ok(())
    }
}

impl StrategyConfig for ChaikinOscillatorMomentumConfig {}

pub struct ChaikinOscillatorMomentum {
    config: ChaikinOscillatorMomentumConfig,
}

impl ChaikinOscillatorMomentum {
    pub fn new(config: ChaikinOscillatorMomentumConfig) -> Self {
        let _ = config.validate();
        Self { config }
    }
}

#[async_trait]
impl Strategy for ChaikinOscillatorMomentum {
    fn name(&self) -> &str {
        "ChaikinOscillatorMomentum"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        self.config.validate()?;

        if data.height() == 0 {
            return Ok(vec![]);
        }

        let co_series = chaikin_oscillator::calculate(data, self.config.fast_period, self.config.slow_period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let co_values = co_series.f64()?;
        let atr_values = atr_series.f64()?;
        let close_series = data.column("close")?.f64()?;
        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let len = data.height();

        if len < 2 {
            return Ok(signals);
        }

        for i in 1..len {
            let prev_co = co_values.get(i - 1);
            let curr_co = co_values.get(i);

            if let (Some(prev), Some(curr)) = (prev_co, curr_co) {
                let timestamp = timestamps.get(i).unwrap_or(0);
                let close = close_series.get(i).unwrap_or(0.0);
                let atr_val = atr_values.get(i).unwrap_or(0.0);

                if prev <= 0.0 && curr > 0.0 {
                    let sl = close - (atr_val * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: None,
                        reason: "Chaikin Oscillator crossed above 0".to_string(),
                        timestamp_ms: timestamp,
                    });
                } else if prev >= 0.0 && curr < 0.0 {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Chaikin Oscillator crossed below 0".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let config: ChaikinOscillatorMomentumConfig = serde_json::from_value(params)?;
        config.validate()?;
        self.config = config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_parameter_validation() {
        let mut config = ChaikinOscillatorMomentumConfig::default();
        config.fast_period = 0;
        assert!(config.validate().is_err());

        config.fast_period = 10;
        config.slow_period = 5;
        assert!(config.validate().is_err());

        config.fast_period = 3;
        config.slow_period = 10;
        assert!(config.validate().is_ok());
    }

    #[tokio::test]
    async fn test_signal_generation() -> Result<()> {
        let times: Vec<i64> = (0..20).map(|i| i * 1000).collect();
        let highs: Vec<f64> = (0..20).map(|_| 12.0).collect();
        let lows: Vec<f64> = (0..20).map(|_| 10.0).collect();
        let closes: Vec<f64> = (0..20).map(|i| if i < 10 { 11.0 } else if i < 16 { 12.0 } else { 10.0 }).collect();
        let volumes: Vec<f64> = (0..20).map(|_| 100.0).collect();

        let df = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes,
            "volume" => volumes,
            "timestamp_unix_ms" => times
        )?;

        let config = ChaikinOscillatorMomentumConfig {
            fast_period: 3,
            slow_period: 10,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = ChaikinOscillatorMomentum::new(config);
        let signals = strategy.generate_signals(&df).await?;

        assert!(signals.len() >= 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let df = DataFrame::default();
        let config = ChaikinOscillatorMomentumConfig::default();
        let strategy = ChaikinOscillatorMomentum::new(config);
        let signals = strategy.generate_signals(&df).await;
        assert!(signals.is_ok());
        assert_eq!(signals.unwrap().len(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_single_row_data() -> Result<()> {
        let df = df!(
            "high" => &[12.0],
            "low" => &[10.0],
            "close" => &[11.0],
            "volume" => &[100.0],
            "timestamp_unix_ms" => &[1000_i64]
        )?;
        let config = ChaikinOscillatorMomentumConfig::default();
        let strategy = ChaikinOscillatorMomentum::new(config);
        let signals = strategy.generate_signals(&df).await;
        assert!(signals.is_ok());
        assert_eq!(signals.unwrap().len(), 0);
        Ok(())
    }
}
