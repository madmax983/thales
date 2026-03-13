use crate::indicators::{atr, stc};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchaffTrendCycleConfig {
    pub macd_fast_period: usize,
    pub macd_slow_period: usize,
    pub stc_period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl SchaffTrendCycleConfig {
    pub fn validate(&self) -> Result<()> {
        if self.macd_fast_period == 0
            || self.macd_slow_period == 0
            || self.stc_period == 0
            || self.atr_period == 0
        {
            anyhow::bail!("Periods must be greater than 0");
        }
        if self.macd_fast_period >= self.macd_slow_period {
            anyhow::bail!("MACD fast period must be strictly less than slow period");
        }
        if self.oversold_threshold >= self.overbought_threshold {
            anyhow::bail!("Oversold threshold must be strictly less than overbought threshold");
        }
        if self.oversold_threshold < 0.0 || self.overbought_threshold > 100.0 {
            anyhow::bail!("Thresholds must be between 0 and 100");
        }
        if self.stop_loss_atr_mult <= 0.0 {
            anyhow::bail!("Stop loss ATR multiplier must be greater than 0");
        }
        if self.symbol.trim().is_empty() {
            anyhow::bail!("Symbol cannot be empty");
        }
        Ok(())
    }
}

impl StrategyConfig for SchaffTrendCycleConfig {}

pub struct SchaffTrendCycle {
    config: SchaffTrendCycleConfig,
}

impl SchaffTrendCycle {
    pub fn new(config: SchaffTrendCycleConfig) -> Self {
        let _ = config.validate(); // Best effort validation on creation
        Self { config }
    }
}

#[async_trait]
impl Strategy for SchaffTrendCycle {
    fn name(&self) -> &str {
        "SchaffTrendCycle"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let stc_series = stc::calculate(
            data,
            self.config.macd_fast_period,
            self.config.macd_slow_period,
            self.config.stc_period,
        )?;
        let stc_arr = stc_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i);
            let stc_curr_opt = stc_arr.get(i);
            let stc_prev_opt = stc_arr.get(i - 1);
            let atr_opt = atr_arr.get(i);

            if let (Some(curr_stc), Some(prev_stc), Some(price)) =
                (stc_curr_opt, stc_prev_opt, price_opt)
            {
                // Long Entry: STC crosses above Oversold Threshold
                if prev_stc <= self.config.oversold_threshold
                    && curr_stc > self.config.oversold_threshold
                {
                    let sl_dist = if let Some(atr_val) = atr_opt {
                        atr_val * self.config.stop_loss_atr_mult
                    } else {
                        price * 0.05
                    };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(price - sl_dist),
                        take_profit: Some(price + (sl_dist * 2.0)),
                        reason: format!(
                            "STC ({:.2}) crossed above oversold threshold ({:.2})",
                            curr_stc, self.config.oversold_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Entry: STC crosses below Overbought Threshold
                if prev_stc >= self.config.overbought_threshold
                    && curr_stc < self.config.overbought_threshold
                {
                    let sl_dist = if let Some(atr_val) = atr_opt {
                        atr_val * self.config.stop_loss_atr_mult
                    } else {
                        price * 0.05
                    };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(price + sl_dist),
                        take_profit: Some(price - (sl_dist * 2.0)),
                        reason: format!(
                            "STC ({:.2}) crossed below overbought threshold ({:.2})",
                            curr_stc, self.config.overbought_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit: STC crosses below overbought
                if prev_stc >= self.config.overbought_threshold
                    && curr_stc < self.config.overbought_threshold
                {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "STC ({:.2}) crossed below overbought ({:.2}) - Exit Long",
                            curr_stc, self.config.overbought_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Exit: STC crosses above oversold
                if prev_stc <= self.config.oversold_threshold
                    && curr_stc > self.config.oversold_threshold
                {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "STC ({:.2}) crossed above oversold ({:.2}) - Exit Short",
                            curr_stc, self.config.oversold_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: SchaffTrendCycleConfig = serde_json::from_value(params)?;
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
    async fn test_schaff_trend_cycle_parameter_validation() {
        // Valid
        let valid_config = SchaffTrendCycleConfig {
            macd_fast_period: 23,
            macd_slow_period: 50,
            stc_period: 10,
            oversold_threshold: 25.0,
            overbought_threshold: 75.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        assert!(valid_config.validate().is_ok());

        // Invalid: Fast >= Slow
        let mut invalid_config = valid_config.clone();
        invalid_config.macd_fast_period = 50;
        invalid_config.macd_slow_period = 50;
        assert!(invalid_config.validate().is_err());

        // Invalid: Oversold >= Overbought
        invalid_config = valid_config.clone();
        invalid_config.oversold_threshold = 80.0;
        invalid_config.overbought_threshold = 80.0;
        assert!(invalid_config.validate().is_err());

        // Invalid: Out of bounds threshold
        invalid_config = valid_config.clone();
        invalid_config.oversold_threshold = -1.0;
        assert!(invalid_config.validate().is_err());

        // Invalid: Zero period
        invalid_config = valid_config.clone();
        invalid_config.stc_period = 0;
        assert!(invalid_config.validate().is_err());
    }

    #[tokio::test]
    async fn test_schaff_trend_cycle_empty_data() -> Result<()> {
        let config = SchaffTrendCycleConfig {
            macd_fast_period: 23,
            macd_slow_period: 50,
            stc_period: 10,
            oversold_threshold: 25.0,
            overbought_threshold: 75.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = SchaffTrendCycle::new(config);
        let df = DataFrame::empty();

        let result = strategy.generate_signals(&df).await;
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_schaff_trend_cycle_signals() -> Result<()> {
        let config = SchaffTrendCycleConfig {
            macd_fast_period: 2,
            macd_slow_period: 5,
            stc_period: 2,
            oversold_threshold: 25.0,
            overbought_threshold: 75.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = SchaffTrendCycle::new(config);

        let mut closes = Vec::new();
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut times = Vec::new();
        for i in 0..50 {
            let val = 100.0 + (i as f64 * 0.2).sin() * 10.0;
            closes.push(val);
            highs.push(val + 1.0);
            lows.push(val - 1.0);
            times.push(i as i64 * 1000);
        }

        let df = df!(
            "timestamp_unix_ms" => times,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(!signals.is_empty());
        Ok(())
    }
}
