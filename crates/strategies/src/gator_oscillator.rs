use crate::indicators::atr;
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration for the Gator Oscillator Strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatorOscillatorConfig {
    /// Period for the fast moving average
    pub fast_period: usize,
    /// Period for the slow moving average
    pub slow_period: usize,
    /// Maximum position size
    pub max_position_size: f64,
    /// Multiplier for ATR to calculate the stop-loss
    pub stop_loss_atr_mult: f64,
    /// Period for ATR calculation
    pub atr_period: usize,
    /// The trading pair symbol
    pub symbol: String,
}

impl Default for GatorOscillatorConfig {
    fn default() -> Self {
        Self {
            fast_period: 5,
            slow_period: 34,
            max_position_size: 100.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for GatorOscillatorConfig {}

/// Gator Oscillator Strategy
///
/// A momentum strategy based on the Awesome Oscillator logic (which Gator Oscillator builds upon).
/// It enters long when the fast moving average crosses above the slow moving average (momentum is positive)
/// and enters short when the fast moving average crosses below the slow moving average (momentum is negative).
pub struct GatorOscillator {
    config: GatorOscillatorConfig,
}

impl GatorOscillator {
    pub fn new(config: GatorOscillatorConfig) -> Result<Self> {
        Ok(Self { config })
    }
}

#[async_trait]
impl Strategy for GatorOscillator {
    fn name(&self) -> &str {
        "GatorOscillator"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(Vec::new());
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate SMAs for fast and slow periods
        let fast_sma = crate::indicators::sma::calculate(data, self.config.fast_period)?;
        let slow_sma = crate::indicators::sma::calculate(data, self.config.slow_period)?;

        let fast_arr = fast_sma.f64()?;
        let slow_arr = slow_sma.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let fast_curr_opt = fast_arr.get(i);
            let fast_prev_opt = fast_arr.get(i - 1);
            let slow_curr_opt = slow_arr.get(i);
            let slow_prev_opt = slow_arr.get(i - 1);
            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(fast),
                Some(prev_fast),
                Some(slow),
                Some(prev_slow),
                Some(price),
                Some(atr),
            ) = (
                fast_curr_opt,
                fast_prev_opt,
                slow_curr_opt,
                slow_prev_opt,
                price_opt,
                atr_opt,
            ) {
                let sl_dist = atr * self.config.stop_loss_atr_mult;
                let size_hint = format!("{:.4}", self.config.max_position_size);

                // Cross above
                if prev_fast <= prev_slow && fast > slow {
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
                        reason: "GatorOscillator fast crossed above slow".to_string(),
                        timestamp_ms: timestamp,
                    });

                    // Exit short if we were short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "GatorOscillator fast crossed above slow".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Cross below
                if prev_fast >= prev_slow && fast < slow {
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
                        reason: "GatorOscillator fast crossed below slow".to_string(),
                        timestamp_ms: timestamp,
                    });

                    // Exit long if we were long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "GatorOscillator fast crossed below slow".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: GatorOscillatorConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_gator_oscillator_empty_data() -> Result<()> {
        let config = GatorOscillatorConfig::default();
        let strategy = GatorOscillator::new(config)?;

        let df_empty = DataFrame::default();
        let signals = strategy.generate_signals(&df_empty).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_gator_oscillator_parameter_validation() -> Result<()> {
        let config = GatorOscillatorConfig {
            fast_period: 2,
            slow_period: 5,
            ..Default::default()
        };

        let strategy = GatorOscillator::new(config)?;
        assert_eq!(strategy.name(), "GatorOscillator");
        assert_eq!(strategy.strategy_type(), StrategyType::Momentum);
        Ok(())
    }

    #[tokio::test]
    async fn test_gator_oscillator_signals() -> Result<()> {
        let config = GatorOscillatorConfig {
            fast_period: 2,
            slow_period: 4,
            atr_period: 2,
            ..Default::default()
        };
        let strategy = GatorOscillator::new(config)?;

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000],
            "open" =>  &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "high" =>  &[15.0, 15.0, 15.0, 15.0, 15.0, 15.0, 15.0],
            "low" =>   &[ 5.0,  5.0,  5.0,  5.0,  5.0,  5.0,  5.0],
            "close" => &[10.0, 12.0, 14.0, 16.0,  8.0,  6.0,  4.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Should generate entry/exit signals given the trend reversal from 16 to 8.
        assert!(!signals.is_empty());

        Ok(())
    }
}
