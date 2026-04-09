use crate::strategy::{Signal, SignalType, Strategy, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::indicators::atr;
use crate::indicators::zlema;

pub struct ZlemaCrossover {
    config: ZlemaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ZlemaCrossoverConfig {
    pub fast_period: usize,
    pub slow_period: usize,
    pub symbol: String,
    pub max_position_size: f64,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
}

impl Default for ZlemaCrossoverConfig {
    fn default() -> Self {
        Self {
            fast_period: 14,
            slow_period: 28,
            symbol: "BTCUSD".to_string(),
            max_position_size: 1.0,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
        }
    }
}

impl ZlemaCrossover {
    pub fn new(config: ZlemaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for ZlemaCrossover {
    fn name(&self) -> &str {
        "ZLEMA Crossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.slow_period {
            return Ok(Vec::new());
        }

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let close_series = data.column("close")?;
        let close_str_series = close_series.cast(&DataType::String)?;
        let close_ca = close_str_series.str()?;

        let fast_zlema = zlema::calculate(data, self.config.fast_period)?;
        let slow_zlema = zlema::calculate(data, self.config.slow_period)?;

        let fast_str = fast_zlema.cast(&DataType::String)?;
        let slow_str = slow_zlema.cast(&DataType::String)?;
        let fast_ca = fast_str.str()?;
        let slow_ca = slow_str.str()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_str = atr_series.cast(&DataType::String)?;
        let atr_ca = atr_str.str()?;

        let mut signals = Vec::new();

        for i in 1..data.height() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let price_str = close_ca.get(i);
            let fast_str_opt = fast_ca.get(i);
            let slow_str_opt = slow_ca.get(i);
            let prev_fast_str = fast_ca.get(i - 1);
            let prev_slow_str = slow_ca.get(i - 1);
            let atr_str_opt = atr_ca.get(i);

            if let (Some(p_str), Some(f_str), Some(s_str), Some(pf_str), Some(ps_str)) = (
                price_str,
                fast_str_opt,
                slow_str_opt,
                prev_fast_str,
                prev_slow_str,
            ) {
                if let (Ok(p_dec), Ok(f_dec), Ok(s_dec), Ok(pf_dec), Ok(ps_dec)) = (
                    Decimal::from_str(p_str),
                    Decimal::from_str(f_str),
                    Decimal::from_str(s_str),
                    Decimal::from_str(pf_str),
                    Decimal::from_str(ps_str),
                ) {
                    let crossed_above = pf_dec <= ps_dec && f_dec > s_dec;
                    let crossed_below = pf_dec >= ps_dec && f_dec < s_dec;

                    let atr_val = if let Some(a_str) = atr_str_opt {
                        Decimal::from_str(a_str)
                            .unwrap_or(p_dec * Decimal::from_f64_retain(0.02).unwrap_or_default())
                    } else {
                        p_dec * Decimal::from_f64_retain(0.02).unwrap_or_default()
                    };

                    let sl_dist = atr_val
                        * Decimal::from_f64_retain(self.config.stop_loss_atr_mult)
                            .unwrap_or_default();

                    let size_hint = format!("{:.4}", self.config.max_position_size);

                    if crossed_above {
                        use rust_decimal::prelude::ToPrimitive;
                        let stop_loss = p_dec - sl_dist;
                        let take_profit = p_dec + (sl_dist * Decimal::TWO);

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: size_hint.clone(),
                            confidence: 0.8,
                            stop_loss: stop_loss.to_f64(),
                            take_profit: take_profit.to_f64(),
                            reason: "ZLEMA Fast crossed above Slow".to_string(),
                            timestamp_ms: timestamp,
                        });
                    } else if crossed_below {
                        use rust_decimal::prelude::ToPrimitive;
                        let stop_loss = p_dec + sl_dist;
                        let take_profit = p_dec - (sl_dist * Decimal::TWO);

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: size_hint.clone(),
                            confidence: 0.8,
                            stop_loss: stop_loss.to_f64(),
                            take_profit: take_profit.to_f64(),
                            reason: "ZLEMA Fast crossed below Slow".to_string(),
                            timestamp_ms: timestamp,
                        });
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: ZlemaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_zlema_crossover_empty_data() -> Result<()> {
        let config = ZlemaCrossoverConfig {
            fast_period: 3,
            slow_period: 5,
            symbol: "TEST".to_string(),
            max_position_size: 1.0,
            atr_period: 3,
            stop_loss_atr_mult: 2.0,
        };
        let strategy = ZlemaCrossover::new(config);
        let df = DataFrame::default();
        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_zlema_crossover_entry_and_exit() -> Result<()> {
        let config = ZlemaCrossoverConfig {
            fast_period: 2,
            slow_period: 4,
            symbol: "TEST".to_string(),
            max_position_size: 1.0,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
        };
        let strategy = ZlemaCrossover::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000, 11000],
            "high" =>  &[10.0, 12.0, 14.0, 16.0, 14.0, 12.0, 10.0, 8.0, 10.0, 12.0, 14.0],
            "low" =>   &[8.0, 10.0, 12.0, 14.0, 12.0, 10.0, 8.0, 6.0, 8.0, 10.0, 12.0],
            "close" => &[9.0, 11.0, 13.0, 15.0, 13.0, 11.0, 9.0, 7.0, 9.0, 11.0, 13.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;
        // Just verify it doesn't panic
        let _ = signals.len();
        Ok(())
    }

    #[tokio::test]
    async fn test_zlema_crossover_extreme_volatility() -> Result<()> {
        let config = ZlemaCrossoverConfig {
            fast_period: 2,
            slow_period: 3,
            symbol: "TEST".to_string(),
            max_position_size: 1.0,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
        };
        let strategy = ZlemaCrossover::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "high" =>  &[100.0, 200.0, 50.0, 300.0, 10.0],
            "low" =>   &[10.0, 50.0, 5.0, 100.0, 1.0],
            "close" => &[50.0, 150.0, 20.0, 250.0, 5.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;
        // Just verify it doesn't panic
        let _ = signals.len();
        Ok(())
    }

    #[tokio::test]
    async fn test_zlema_parameter_validation() -> Result<()> {
        let mut config = ZlemaCrossoverConfig {
            fast_period: 3,
            slow_period: 5,
            symbol: "TEST".to_string(),
            max_position_size: 1.0,
            atr_period: 3,
            stop_loss_atr_mult: 2.0,
        };
        let mut strategy = ZlemaCrossover::new(config.clone());

        config.fast_period = 4;
        let new_params = serde_json::to_value(&config)?;

        let update_res = strategy.update_params(new_params).await;
        assert!(update_res.is_ok());
        assert_eq!(strategy.config.fast_period, 4);

        Ok(())
    }
}
