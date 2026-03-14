use crate::indicators::{atr, kama};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::{bail, Result};
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KamaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub short_fast_ema_period: usize,
    pub short_slow_ema_period: usize,
    pub long_fast_ema_period: usize,
    pub long_slow_ema_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for KamaCrossoverConfig {
    fn default() -> Self {
        Self {
            short_period: 10,
            long_period: 30,
            short_fast_ema_period: 2,
            short_slow_ema_period: 30,
            long_fast_ema_period: 2,
            long_slow_ema_period: 30,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for KamaCrossoverConfig {}

impl KamaCrossoverConfig {
    pub fn validate(&self) -> Result<()> {
        if self.short_period == 0 || self.long_period == 0 {
            bail!("Periods must be greater than 0");
        }
        if self.short_period >= self.long_period {
            bail!("Short period must be less than long period");
        }
        if self.atr_period == 0 {
            bail!("ATR period must be greater than 0");
        }
        Ok(())
    }
}

pub struct KamaCrossover {
    config: KamaCrossoverConfig,
}

impl KamaCrossover {
    pub fn new(config: KamaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for KamaCrossover {
    fn name(&self) -> &str {
        "KamaCrossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        self.config.validate()?;

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let short_kama_series = kama::calculate(
            data,
            self.config.short_period,
            self.config.short_fast_ema_period,
            self.config.short_slow_ema_period,
        )?;
        let long_kama_series = kama::calculate(
            data,
            self.config.long_period,
            self.config.long_fast_ema_period,
            self.config.long_slow_ema_period,
        )?;

        let short_kama = short_kama_series.f64()?;
        let long_kama = long_kama_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);

            let s_curr_opt = short_kama.get(i).and_then(Decimal::from_f64_retain);
            let l_curr_opt = long_kama.get(i).and_then(Decimal::from_f64_retain);
            let s_prev_opt = short_kama.get(i - 1).and_then(Decimal::from_f64_retain);
            let l_prev_opt = long_kama.get(i - 1).and_then(Decimal::from_f64_retain);

            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(sc), Some(lc), Some(sp), Some(lp), Some(price)) =
                (s_curr_opt, l_curr_opt, s_prev_opt, l_prev_opt, price_opt)
            {
                // Bearish Crossover (Exit Long / Entry Short equivalent - but we focus on Long Exit)
                if sc < lc && sp >= lp {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bearish KAMA Crossover: Short {} < Long {}",
                            sc.round_dp(2),
                            lc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Bullish Crossover (Entry Long)
                if sc > lc && sp <= lp {
                    let sl = if let Some(atr_val) = atr_opt {
                        price - (atr_val * atr_mult_dec)
                    } else {
                        // Fallback SL if ATR is not available yet
                        price * Decimal::from_f64_retain(0.95).unwrap()
                    };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "Bullish KAMA Crossover: Short {} > Long {}",
                            sc.round_dp(2),
                            lc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: KamaCrossoverConfig = serde_json::from_value(params)?;
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
    async fn test_kama_crossover_signals() -> Result<()> {
        let config = KamaCrossoverConfig {
            short_period: 2,
            long_period: 3,
            short_fast_ema_period: 2,
            short_slow_ema_period: 30,
            long_fast_ema_period: 2,
            long_slow_ema_period: 30,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = KamaCrossover::new(config);

        // Prices needed for KAMA calculation
        let closes = vec![
            10.0, 10.5, 10.2, 10.8, 11.0, 11.5, 11.2, 11.8, 12.0, 12.5, 11.0, 9.0,
        ];
        let highs = vec![
            10.5, 11.0, 10.7, 11.3, 11.5, 12.0, 11.7, 12.3, 12.5, 13.0, 12.0, 10.0,
        ];
        let lows = vec![
            9.5, 10.0, 9.7, 10.3, 10.5, 11.0, 10.7, 11.3, 11.5, 12.0, 10.0, 8.0,
        ];
        let timestamps: Vec<i64> = (0..closes.len()).map(|i| (i as i64) * 1000).collect();

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        assert!(!signals.is_empty());

        // Check if there's at least one entry signal
        let entry_signal = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry_signal.is_some());

        let entry = entry_signal.unwrap();
        assert_eq!(entry.side, "buy");
        assert!(entry.stop_loss.is_some());

        Ok(())
    }

    #[test]
    fn test_parameter_validation() -> Result<()> {
        let mut config = KamaCrossoverConfig::default();
        assert!(config.validate().is_ok());

        config.short_period = 30;
        config.long_period = 10;
        let res = config.validate();
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "Short period must be less than long period"
        );

        config.short_period = 0;
        config.long_period = 30;
        let res2 = config.validate();
        assert!(res2.is_err());
        assert_eq!(
            res2.unwrap_err().to_string(),
            "Periods must be greater than 0"
        );

        Ok(())
    }
}
