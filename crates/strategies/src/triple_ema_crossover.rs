use crate::indicators::{atr, ema};
use crate::strategy::{Signal, SignalType, Strategy};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

pub struct TripleEmaCrossover {
    config: TripleEmaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TripleEmaCrossoverConfig {
    pub short_period: usize,
    pub medium_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl TripleEmaCrossoverConfig {
    pub fn validate(&self) -> Result<()> {
        if self.short_period == 0
            || self.medium_period == 0
            || self.long_period == 0
            || self.atr_period == 0
        {
            anyhow::bail!("Periods must be greater than 0");
        }
        if self.short_period >= self.medium_period || self.medium_period >= self.long_period {
            anyhow::bail!("Periods must be strictly increasing: short < medium < long");
        }
        if self.stop_loss_atr_mult <= 0.0 {
            anyhow::bail!("stop_loss_atr_mult must be greater than 0");
        }
        if self.symbol.is_empty() {
            anyhow::bail!("Symbol must not be empty");
        }
        Ok(())
    }
}

impl TripleEmaCrossover {
    pub fn new(config: TripleEmaCrossoverConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }
}

#[async_trait]
impl Strategy for TripleEmaCrossover {
    fn name(&self) -> &str {
        "TripleEmaCrossover"
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.is_empty() {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?;
        if close_series.null_count() > 0 {
            anyhow::bail!("Close column contains null values");
        }

        let short_ema = ema::calculate(data, self.config.short_period)?;
        let medium_ema = ema::calculate(data, self.config.medium_period)?;
        let long_ema = ema::calculate(data, self.config.long_period)?;
        let atr = atr::calculate(data, self.config.atr_period)?;

        let short_ema_f64 = short_ema.f64()?;
        let medium_ema_f64 = medium_ema.f64()?;
        let long_ema_f64 = long_ema.f64()?;
        let atr_f64 = atr.f64()?;
        let close_f64 = close_series.f64()?;

        let timestamp_series = data.column("timestamp_unix_ms").ok();
        let timestamp_i64 = match timestamp_series {
            Some(series) => Some(series.i64()?),
            None => None,
        };

        let mut signals = Vec::new();
        let mut in_long = false;
        let mut in_short = false;

        let len = short_ema_f64.len();

        for i in 1..len {
            let short = short_ema_f64.get(i);
            let medium = medium_ema_f64.get(i);
            let long = long_ema_f64.get(i);
            let prev_short = short_ema_f64.get(i - 1);
            let prev_medium = medium_ema_f64.get(i - 1);
            let prev_long = long_ema_f64.get(i - 1);

            if let (Some(s), Some(m), Some(l), Some(ps), Some(pm), Some(pl)) =
                (short, medium, long, prev_short, prev_medium, prev_long)
            {
                let is_bullish = s > m && m > l;
                let prev_is_bullish = ps > pm && pm > pl;
                let is_bearish = s < m && m < l;
                let prev_is_bearish = ps < pm && pm < pl;

                let current_atr = atr_f64.get(i).unwrap_or(0.0);
                let current_close = close_f64.get(i).unwrap_or(0.0);
                let current_ts = match timestamp_i64 {
                    Some(ts) => ts.get(i).unwrap_or(0),
                    None => 0,
                };

                // First evaluate exits
                if in_long && s < m {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.9,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Short EMA crossed below Medium EMA (Exit Long)".to_string(),
                        timestamp_ms: current_ts,
                    });
                    in_long = false;
                }

                if in_short && s > m {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.9,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Short EMA crossed above Medium EMA (Exit Short)".to_string(),
                        timestamp_ms: current_ts,
                    });
                    in_short = false;
                }

                // Then evaluate entries
                if !in_long && is_bullish && !prev_is_bullish {
                    let sl = current_close - (current_atr * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: None,
                        reason: "Short EMA > Medium EMA > Long EMA".to_string(),
                        timestamp_ms: current_ts,
                    });
                    in_long = true;
                    in_short = false; // Failsafe
                } else if !in_short && is_bearish && !prev_is_bearish {
                    let sl = current_close + (current_atr * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: None,
                        reason: "Short EMA < Medium EMA < Long EMA".to_string(),
                        timestamp_ms: current_ts,
                    });
                    in_short = true;
                    in_long = false; // Failsafe
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: TripleEmaCrossoverConfig = serde_json::from_value(params)?;
        new_config.validate()?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_triple_ema_crossover_parameter_validation() {
        let valid_config = TripleEmaCrossoverConfig {
            short_period: 9,
            medium_period: 21,
            long_period: 50,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "BTC".to_string(),
        };
        assert!(valid_config.validate().is_ok());

        let invalid_periods = TripleEmaCrossoverConfig {
            short_period: 21,
            medium_period: 9,
            long_period: 50,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "BTC".to_string(),
        };
        assert!(invalid_periods.validate().is_err());
    }

    #[tokio::test]
    async fn test_triple_ema_crossover_empty_data() -> Result<()> {
        let df = DataFrame::empty();
        let strategy = TripleEmaCrossover::new(TripleEmaCrossoverConfig {
            short_period: 9,
            medium_period: 21,
            long_period: 50,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "BTC".to_string(),
        })?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_triple_ema_crossover_realistic_market_data() -> Result<()> {
        // Create 100 data points simulating an uptrend, then downtrend
        let mut close_data = Vec::with_capacity(200);
        let mut high_data = Vec::with_capacity(200);
        let mut low_data = Vec::with_capacity(200);

        // Need to give enough time for EMAs to settle, so we extend the period
        for i in 0..100 {
            close_data.push(100.0 + (i as f64) * 2.0); // Steady uptrend
            high_data.push(100.0 + (i as f64) * 2.0 + 1.0);
            low_data.push(100.0 + (i as f64) * 2.0 - 1.0);
        }
        for i in 100..200 {
            close_data.push(300.0 - ((i - 100) as f64) * 2.0); // Steady downtrend
            high_data.push(300.0 - ((i - 100) as f64) * 2.0 + 1.0);
            low_data.push(300.0 - ((i - 100) as f64) * 2.0 - 1.0);
        }

        let df = df!(
            "close" => &close_data,
            "high" => &high_data,
            "low" => &low_data,
        )?;

        let strategy = TripleEmaCrossover::new(TripleEmaCrossoverConfig {
            short_period: 3,
            medium_period: 6,
            long_period: 12,
            stop_loss_atr_mult: 1.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        })?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(!signals.is_empty());

        let mut has_buy = false;
        let mut has_sell = false;
        let mut has_exit = false;

        for sig in signals {
            if sig.signal_type == SignalType::Entry && sig.side == "buy" {
                has_buy = true;
            }
            if sig.signal_type == SignalType::Entry && sig.side == "sell" {
                has_sell = true;
            }
            if sig.signal_type == SignalType::Exit {
                has_exit = true;
            }
        }

        assert!(
            has_buy || has_sell || has_exit,
            "Expected at least one signal"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_triple_ema_crossover_known_values() -> Result<()> {
        // Small dataset to hit known values
        let close_data = vec![
            10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 12.0, 14.0, 16.0, 18.0,
            20.0, 22.0, 24.0, 26.0, 28.0, 30.0,
        ];
        let df = df!(
            "close" => &close_data,
            "high" => &close_data,
            "low" => &close_data,
        )?;

        let strategy = TripleEmaCrossover::new(TripleEmaCrossoverConfig {
            short_period: 2,
            medium_period: 4,
            long_period: 6,
            stop_loss_atr_mult: 1.0,
            atr_period: 4,
            symbol: "KNOWN".to_string(),
        })?;

        let signals = strategy.generate_signals(&df).await?;

        // Wait for EMAs to order themselves after the flatline starts increasing
        let mut found_long_entry = false;
        for s in &signals {
            if s.signal_type == SignalType::Entry && s.side == "buy" && s.reason.contains(">") {
                found_long_entry = true;
            }
        }

        assert!(
            found_long_entry,
            "Expected a long entry when trend starts increasing"
        );
        Ok(())
    }
}
