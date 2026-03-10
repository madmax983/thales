use crate::indicators::{atr, tema};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemaCrossoverConfig {
    pub short_window: usize,
    pub long_window: usize,
    pub stop_loss_pct: f64,
    pub atr_period: usize,
    pub atr_mult: f64,
    pub symbol: String,
}

impl Default for TemaCrossoverConfig {
    fn default() -> Self {
        Self {
            short_window: 9,
            long_window: 21,
            stop_loss_pct: 0.05,
            atr_period: 14,
            atr_mult: 2.0,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for TemaCrossoverConfig {}

impl TemaCrossoverConfig {
    pub fn validate(&self) -> Result<()> {
        if self.short_window >= self.long_window {
            anyhow::bail!("short_window must be less than long_window");
        }
        if self.short_window == 0 || self.long_window == 0 {
            anyhow::bail!("windows must be greater than 0");
        }
        if self.atr_period == 0 {
            anyhow::bail!("atr_period must be greater than 0");
        }
        if self.stop_loss_pct < 0.0 || self.stop_loss_pct > 1.0 {
            anyhow::bail!("stop_loss_pct must be between 0.0 and 1.0");
        }
        Ok(())
    }
}


pub struct TemaCrossover {
    config: TemaCrossoverConfig,
}

impl TemaCrossover {
    pub fn new(config: TemaCrossoverConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
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

        let short_tema_series = tema::calculate(data, self.config.short_window)?;
        let long_tema_series = tema::calculate(data, self.config.long_window)?;

        let short_tema = short_tema_series.f64()?;
        let long_tema = long_tema_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_pct_dec =
            Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.atr_mult).unwrap_or(Decimal::new(2, 0)); // Default 2.0 if missing

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);

            // Ensure we have TEMA values
            let s_curr_opt = short_tema.get(i).and_then(Decimal::from_f64_retain);
            let l_curr_opt = long_tema.get(i).and_then(Decimal::from_f64_retain);
            let s_prev_opt = short_tema.get(i - 1).and_then(Decimal::from_f64_retain);
            let l_prev_opt = long_tema.get(i - 1).and_then(Decimal::from_f64_retain);

            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(sc), Some(lc), Some(sp), Some(lp), Some(price)) =
                (s_curr_opt, l_curr_opt, s_prev_opt, l_prev_opt, price_opt)
            {
                // Bearish Crossover (Exit) - Stateless
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
                            "Bearish Crossover: Short {} < Long {}",
                            sc.round_dp(2),
                            lc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Bullish Crossover (Entry) - Stateless
                if sc > lc && sp <= lp {
                    // Use ATR-based SL if available, else Fallback %
                    let sl = if let Some(atr_val) = atr_opt {
                        price - (atr_val * atr_mult_dec)
                    } else {
                        price * (one_dec - stop_loss_pct_dec)
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
                            "Bullish Crossover: Short {} > Long {}",
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
        let new_config: TemaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_parameter_validation() {
        let config_invalid_windows = TemaCrossoverConfig {
            short_window: 20,
            long_window: 10,
            ..Default::default()
        };
        assert!(config_invalid_windows.validate().is_err());

        let config_valid = TemaCrossoverConfig {
            short_window: 10,
            long_window: 20,
            ..Default::default()
        };
        assert!(config_valid.validate().is_ok());

        let config_invalid_atr = TemaCrossoverConfig {
            atr_period: 0,
            ..Default::default()
        };
        assert!(config_invalid_atr.validate().is_err());
    }


    #[tokio::test]
    async fn test_tema_crossover_signals() -> Result<()> {
        let config = TemaCrossoverConfig {
            short_window: 2,
            long_window: 4,
            stop_loss_pct: 0.2,
            atr_period: 2,
            atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = TemaCrossover::new(config).unwrap();

        // Needs enough data for EMA of EMA of EMA to warm up.
        // Let's create a flat market, then a spike up, then a drop down.
        let mut closes = vec![10.0; 20];
        closes.extend_from_slice(&[10.0, 11.0, 13.0, 15.0, 16.0, 14.0, 11.0, 9.0, 9.0, 9.0]);

        let mut highs = vec![10.5; 20];
        highs.extend_from_slice(&[10.5, 11.5, 13.5, 15.5, 16.5, 14.5, 11.5, 9.5, 9.5, 9.5]);

        let mut lows = vec![9.5; 20];
        lows.extend_from_slice(&[9.5, 10.5, 12.5, 14.5, 15.5, 13.5, 10.5, 8.5, 8.5, 8.5]);

        let timestamps: Vec<i64> = (0..30).map(|i| i * 1000).collect();

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect at least one entry and one exit
        assert!(!signals.is_empty());

        Ok(())
    }
}
