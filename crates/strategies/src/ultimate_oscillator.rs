use crate::indicators::{atr, ultimate_oscillator};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::{bail, Result};
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UltimateOscillatorConfig {
    pub period1: usize,
    pub period2: usize,
    pub period3: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub max_position_size: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl UltimateOscillatorConfig {
    pub fn validate(&self) -> Result<()> {
        if self.period1 == 0 || self.period2 == 0 || self.period3 == 0 {
            bail!("Periods must be strictly positive");
        }
        if self.period1 >= self.period2 || self.period2 >= self.period3 {
            bail!("Periods must be strictly increasing: period1 < period2 < period3");
        }
        if self.oversold_threshold < 0.0 || self.oversold_threshold > 100.0 {
            bail!("Oversold threshold must be between 0 and 100");
        }
        if self.overbought_threshold < 0.0 || self.overbought_threshold > 100.0 {
            bail!("Overbought threshold must be between 0 and 100");
        }
        if self.oversold_threshold >= self.overbought_threshold {
            bail!("Oversold threshold must be strictly less than overbought threshold");
        }
        if self.stop_loss_atr_mult <= 0.0 {
            bail!("ATR multiplier for stop loss must be greater than zero");
        }
        if self.max_position_size <= 0.0 {
            bail!("Max position size must be greater than zero");
        }
        if self.atr_period == 0 {
            bail!("ATR period must be greater than zero");
        }
        Ok(())
    }
}

impl StrategyConfig for UltimateOscillatorConfig {}

pub struct UltimateOscillator {
    config: UltimateOscillatorConfig,
}

impl UltimateOscillator {
    pub fn new(config: UltimateOscillatorConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }
}

#[async_trait]
impl Strategy for UltimateOscillator {
    fn name(&self) -> &str {
        "UltimateOscillator"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.period3 + 1 {
            return Ok(vec![]);
        }

        let uo = ultimate_oscillator::calculate(
            data,
            self.config.period1,
            self.config.period2,
            self.config.period3,
        )?;
        let mut uo_series = uo.clone().into_series();
        uo_series.rename("uo".into());
        let atr_vals = atr::calculate(data, self.config.atr_period)?;
        let mut atr_series_base = atr_vals.clone().into_series();
        atr_series_base.rename("atr".into());

        let df = data.clone()
            .lazy()
            .with_columns(vec![
                lit(uo_series).alias("uo"),
                lit(atr_series_base).alias("atr"),
            ])
            .with_columns(vec![
                col("uo").shift(lit(1)).alias("prev_uo"),
            ])
            .collect()?;

        let mut signals = Vec::new();

        let uo_series = df.column("uo")?.f64()?;
        let prev_uo_series = df.column("prev_uo")?.f64()?;
        let close_series = df.column("close")?.f64()?;
        let atr_series = df.column("atr")?.f64()?;
        let timestamps = df.column("timestamp_unix_ms")?.i64()?;

        let iter = uo_series.into_iter()
            .zip(prev_uo_series.into_iter())
            .zip(close_series.into_iter())
            .zip(atr_series.into_iter())
            .zip(timestamps.into_iter())
            .skip(self.config.period3);

        for ((((current_uo, prev_uo), current_close), current_atr), current_ts) in iter {
            if let (
                Some(current_uo),
                Some(prev_uo),
                Some(current_close),
                Some(current_atr),
                Some(current_ts),
            ) = (current_uo, prev_uo, current_close, current_atr, current_ts)
            {
                // Entry Logic (Long)
                if prev_uo <= self.config.oversold_threshold && current_uo > self.config.oversold_threshold {
                    let stop_loss = current_close - (current_atr * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: "UO Crossed Above Oversold Threshold".to_string(),
                        timestamp_ms: current_ts,
                    });
                }

                // Exit Logic (Long)
                if prev_uo >= self.config.overbought_threshold && current_uo < self.config.overbought_threshold {
                     signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "UO Crossed Below Overbought Threshold".to_string(),
                        timestamp_ms: current_ts,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: UltimateOscillatorConfig = serde_json::from_value(params)?;
        new_config.validate()?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_ultimate_oscillator_entry_signals() -> Result<()> {
        let size = 100;
        let config = UltimateOscillatorConfig {
            period1: 7,
            period2: 14,
            period3: 28,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            stop_loss_atr_mult: 2.0,
            max_position_size: 100.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = UltimateOscillator::new(config)?;

        // Generate known data that definitely causes a UO signal
        let mut close_prices: Vec<f64> = vec![100.0; size];
        let mut high_prices: Vec<f64> = vec![105.0; size];
        let mut low_prices: Vec<f64> = vec![95.0; size];
        let timestamps: Vec<i64> = (0..size).map(|i| (i * 1000) as i64).collect();

        // Induce an oversold cross
        for i in 20..40 {
            close_prices[i] = 50.0; // Price drops
            low_prices[i] = 45.0;
        }
        for i in 40..60 {
            close_prices[i] = 100.0; // Price rebounds sharply
            high_prices[i] = 105.0;
        }

        let df = df!(
            "high" => &high_prices,
            "low" => &low_prices,
            "close" => &close_prices,
            "timestamp_unix_ms" => &timestamps
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let has_entry = signals.iter().any(|s| s.signal_type == SignalType::Entry);
        assert!(has_entry, "Expected entry signals to be generated");
        Ok(())
    }

    #[tokio::test]
    async fn test_ultimate_oscillator_exit_signals() -> Result<()> {
        let size = 100;
        let config = UltimateOscillatorConfig {
            period1: 7,
            period2: 14,
            period3: 28,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            stop_loss_atr_mult: 2.0,
            max_position_size: 100.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = UltimateOscillator::new(config)?;

        let mut close_prices: Vec<f64> = vec![100.0; size];
        let mut high_prices: Vec<f64> = vec![105.0; size];
        let mut low_prices: Vec<f64> = vec![95.0; size];
        let timestamps: Vec<i64> = (0..size).map(|i| (i * 1000) as i64).collect();

        // Induce an overbought cross (price spikes then drops)
        for i in 20..40 {
            close_prices[i] = 150.0; // Price spikes
            high_prices[i] = 155.0;
        }
        for i in 40..60 {
            close_prices[i] = 100.0; // Price drops
            low_prices[i] = 95.0;
        }

        let df = df!(
            "high" => &high_prices,
            "low" => &low_prices,
            "close" => &close_prices,
            "timestamp_unix_ms" => &timestamps
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let has_exit = signals.iter().any(|s| s.signal_type == SignalType::Exit);
        assert!(has_exit, "Expected exit signals to be generated");
        Ok(())
    }

    #[tokio::test]
    async fn test_ultimate_oscillator_empty_data() -> Result<()> {
        let config = UltimateOscillatorConfig {
            period1: 7,
            period2: 14,
            period3: 28,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            stop_loss_atr_mult: 2.0,
            max_position_size: 100.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = UltimateOscillator::new(config)?;

        let df = df!(
            "high" => Vec::<f64>::new(),
            "low" => Vec::<f64>::new(),
            "close" => Vec::<f64>::new(),
            "timestamp_unix_ms" => Vec::<i64>::new()
        )?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_ultimate_oscillator_update_params() -> Result<()> {
        let mut config = UltimateOscillatorConfig {
            period1: 7,
            period2: 14,
            period3: 28,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            stop_loss_atr_mult: 2.0,
            max_position_size: 100.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let mut strategy = UltimateOscillator::new(config.clone())?;

        config.oversold_threshold = 25.0;
        let json_params = serde_json::to_value(&config)?;
        strategy.update_params(json_params).await?;

        assert_eq!(strategy.config.oversold_threshold, 25.0);
        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let mut config = UltimateOscillatorConfig {
            period1: 7,
            period2: 14,
            period3: 28,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            stop_loss_atr_mult: 2.0,
            max_position_size: 100.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        // Valid config
        assert!(config.validate().is_ok());

        // Invalid periods (not increasing)
        config.period1 = 14;
        config.period2 = 7;
        assert!(config.validate().is_err());

        // Reset periods
        config.period1 = 7;
        config.period2 = 14;

        // Invalid thresholds
        config.oversold_threshold = 80.0;
        config.overbought_threshold = 70.0;
        assert!(config.validate().is_err());

        config.oversold_threshold = -10.0;
        assert!(config.validate().is_err());

        // Reset thresholds
        config.oversold_threshold = 30.0;
        config.overbought_threshold = 70.0;

        // Invalid ATR multiplier
        config.stop_loss_atr_mult = 0.0;
        assert!(config.validate().is_err());

        config.stop_loss_atr_mult = 2.0;

        // Invalid position size
        config.max_position_size = -10.0;
        assert!(config.validate().is_err());
    }
}
