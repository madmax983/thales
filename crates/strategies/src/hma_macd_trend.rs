//! The HMA + MACD Strategy
//!
//! Combines the Hull Moving Average with MACD.
//!
use crate::indicators::{atr, hma, macd};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmaMacdTrendConfig {
    pub hma_period: usize,
    pub macd_fast_period: usize,
    pub macd_slow_period: usize,
    pub macd_signal_period: usize,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}

impl Default for HmaMacdTrendConfig {
    fn default() -> Self {
        Self {
            hma_period: 21,
            macd_fast_period: 12,
            macd_slow_period: 26,
            macd_signal_period: 9,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        }
    }
}

impl StrategyConfig for HmaMacdTrendConfig {}

pub struct HmaMacdTrend {
    config: HmaMacdTrendConfig,
}

impl HmaMacdTrend {
    pub fn new(config: HmaMacdTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for HmaMacdTrend {
    fn name(&self) -> &str {
        "HmaMacdTrend"
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

        // Calculate HMA
        let hma_series = hma::calculate(data, self.config.hma_period)?;
        let hma_arr = hma_series.f64()?;

        // Calculate MACD
        let macd_result = macd::calculate(
            data,
            self.config.macd_fast_period,
            self.config.macd_slow_period,
            self.config.macd_signal_period,
        )?;
        let _macd_line = macd_result.0;
        let _macd_signal = macd_result.1;
        let macd_hist_series = macd_result.2;
        let macd_hist = macd_hist_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);

            let hma_curr = hma_arr.get(i).and_then(Decimal::from_f64_retain);

            let hist_curr = macd_hist.get(i).and_then(Decimal::from_f64_retain);
            let hist_prev = macd_hist.get(i - 1).and_then(Decimal::from_f64_retain);

            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(price), Some(hma), Some(hc), Some(hp)) =
                (price_opt, hma_curr, hist_curr, hist_prev)
            {
                // Long Entry
                if price > hma && hc > Decimal::ZERO && hp <= Decimal::ZERO {
                    let sl: Decimal = if let Some(atr_val) = atr_opt {
                        price - (atr_val * atr_mult_dec)
                    } else {
                        price * Decimal::from_f64_retain(0.95).unwrap_or(Decimal::ZERO)
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
                            "Long Entry: Close > HMA ({}) and MACD Hist {} > 0",
                            hma.round_dp(2),
                            hc.round_dp(4)
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit
                if price < hma || hc < Decimal::ZERO {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Long Exit: Close < HMA or MACD Hist < 0".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Entry
                if price < hma && hc < Decimal::ZERO && hp >= Decimal::ZERO {
                    let sl: Decimal = if let Some(atr_val) = atr_opt {
                        price + (atr_val * atr_mult_dec)
                    } else {
                        price * Decimal::from_f64_retain(1.05).unwrap_or(Decimal::ZERO)
                    };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "Short Entry: Close < HMA ({}) and MACD Hist {} < 0",
                            hma.round_dp(2),
                            hc.round_dp(4)
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Exit
                if price > hma || hc > Decimal::ZERO {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Short Exit: Close > HMA or MACD Hist > 0".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: HmaMacdTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> Result<DataFrame> {
        // Create 100 rows of test data
        let mut closes = Vec::new();
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut times = Vec::new();

        let mut price = 100.0;
        for i in 0..100 {
            // Trend up then trend down
            if i < 50 {
                price += 1.0;
            } else {
                price -= 1.0;
            }

            closes.push(price);
            highs.push(price + 2.0);
            lows.push(price - 2.0);
            times.push(i as i64 * 1000);
        }

        let df = df!(
            "timestamp_unix_ms" => times,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        // We need to cast columns to Float64 explicitly just in case df macro inferred them wrong
        let mut df = df.clone();
        df.try_apply("close", |s| s.cast(&DataType::Float64))?;
        df.try_apply("high", |s| s.cast(&DataType::Float64))?;
        df.try_apply("low", |s| s.cast(&DataType::Float64))?;

        Ok(df)
    }

    #[tokio::test]
    async fn test_entry_signal_generation() -> Result<()> {
        let config = HmaMacdTrendConfig {
            hma_period: 9,
            macd_fast_period: 5,
            macd_slow_period: 10,
            macd_signal_period: 3,
            atr_period: 5,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };

        let strategy = HmaMacdTrend::new(config);
        let df = create_test_data()?;
        let signals = strategy.generate_signals(&df).await?;

        // Ensure we got some signals
        assert!(!signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_exit_signal_generation() -> Result<()> {
        let config = HmaMacdTrendConfig {
            hma_period: 9,
            macd_fast_period: 5,
            macd_slow_period: 10,
            macd_signal_period: 3,
            atr_period: 5,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };

        let strategy = HmaMacdTrend::new(config);
        let df = create_test_data()?;
        let signals = strategy.generate_signals(&df).await?;

        // We should have exits in the signals
        let has_exit = signals.iter().any(|s| s.signal_type == SignalType::Exit);
        assert!(has_exit);
        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = HmaMacdTrend::new(HmaMacdTrendConfig::default());
        let new_params = serde_json::json!({
            "hma_period": 14,
            "macd_fast_period": 8,
            "macd_slow_period": 21,
            "macd_signal_period": 5,
            "atr_period": 10,
            "stop_loss_atr_mult": 1.5,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.hma_period, 14);
        assert_eq!(strategy.config.symbol, "BTCUSD");
        Ok(())
    }
}
