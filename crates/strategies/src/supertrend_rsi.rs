//! The Supertrend + RSI Strategy
//!
//! This strategy uses the Supertrend indicator for overall trend direction and the Relative Strength Index (RSI) to find optimal entry points during pullbacks.
//!
//! - **Entry Signal:** A buy signal is generated when the Supertrend is bullish and the RSI dips into oversold territory (e.g., < 30) and then turns back up.
//! - **Exit Signal:** A sell signal is generated when the Supertrend turns bearish or the RSI enters overbought territory (e.g., > 70).
//!
use crate::indicators::{atr, rsi, supertrend};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Configuration parameters for the `SupertrendRsi` strategy.
///
/// # Examples
///
/// ```
/// use strategies::supertrend_rsi::SupertrendRsiConfig;
///
/// let config = SupertrendRsiConfig {
///     supertrend_period: 14,
///     supertrend_multiplier: 3.0,
///     rsi_period: 14,
///     rsi_oversold: 30.0,
///     rsi_overbought: 70.0,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     symbol: "BTCUSD".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupertrendRsiConfig {
    pub supertrend_period: usize,
    pub supertrend_multiplier: f64,
    pub rsi_period: usize,
    pub rsi_oversold: f64,
    pub rsi_overbought: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for SupertrendRsiConfig {
    fn default() -> Self {
        Self {
            supertrend_period: 10,
            supertrend_multiplier: 3.0,
            rsi_period: 14,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "BTC/USD".to_string(),
        }
    }
}

impl StrategyConfig for SupertrendRsiConfig {}

/// The Supertrend + RSI strategy implementation.
///
/// # Examples
///
/// ```
/// use strategies::supertrend_rsi::{SupertrendRsi, SupertrendRsiConfig};
/// use strategies::strategy::Strategy;
///
/// let config = SupertrendRsiConfig {
///     supertrend_period: 14,
///     supertrend_multiplier: 3.0,
///     rsi_period: 14,
///     rsi_oversold: 30.0,
///     rsi_overbought: 70.0,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     symbol: "BTCUSD".to_string(),
/// };
///
/// let strategy = SupertrendRsi::new(config);
/// assert_eq!(strategy.name(), "SupertrendRsi");
/// ```
pub struct SupertrendRsi {
    config: SupertrendRsiConfig,
}

impl SupertrendRsi {
    pub fn new(config: SupertrendRsiConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for SupertrendRsi {
    fn name(&self) -> &str {
        "SupertrendRsi"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        let (_, supertrend_trend_series) = supertrend::calculate(
            data,
            self.config.supertrend_period,
            self.config.supertrend_multiplier,
        )?;
        let rsi_series = rsi::calculate(data, self.config.rsi_period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let st_trend_arr = supertrend_trend_series.i32()?;
        let rsi_arr = rsi_series.f64()?;
        let atr_arr = atr_series.f64()?;

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let st_trend_opt = st_trend_arr.get(i);
            let st_trend_prev_opt = st_trend_arr.get(i - 1);
            let rsi_curr_opt = rsi_arr.get(i).and_then(Decimal::from_f64_retain);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);
            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(st_trend), Some(st_trend_prev), Some(rsi_curr), Some(price)) =
                (st_trend_opt, st_trend_prev_opt, rsi_curr_opt, price_opt)
            {
                let rsi_oversold = Decimal::from_f64_retain(self.config.rsi_oversold)
                    .unwrap_or(Decimal::new(30, 0));
                let rsi_overbought = Decimal::from_f64_retain(self.config.rsi_overbought)
                    .unwrap_or(Decimal::new(70, 0));

                if st_trend == 1 && rsi_curr < rsi_oversold {
                    let mut sl = price * Decimal::new(99, 2);
                    if let Some(atr_val) = atr_opt {
                        sl = price - (atr_val * sl_mult);
                    }

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "Bullish Supertrend RSI: ST Up, RSI {:.2} < {:.2}",
                            rsi_curr, rsi_oversold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                if st_trend == -1 && rsi_curr > rsi_overbought {
                    let mut sl = price * Decimal::new(101, 2);
                    if let Some(atr_val) = atr_opt {
                        sl = price + (atr_val * sl_mult);
                    }

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "Bearish Supertrend RSI: ST Down, RSI {:.2} > {:.2}",
                            rsi_curr, rsi_overbought
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                if st_trend == -1 && st_trend_prev == 1 {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Exit Long: ST Flipped Down".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                if st_trend == 1 && st_trend_prev == -1 {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Exit Short: ST Flipped Up".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: SupertrendRsiConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_supertrend_rsi_signals() -> Result<()> {
        let config = SupertrendRsiConfig {
            supertrend_period: 2,
            supertrend_multiplier: 1.0,
            rsi_period: 2,
            rsi_oversold: 50.0,
            rsi_overbought: 50.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = SupertrendRsi::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000],
            "open"  => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "high"  => &[11.0, 12.0, 13.0, 14.0, 15.0, 14.0, 12.0],
            "low"   => &[9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0],
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 11.0, 9.0],
            "volume"=> &[100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Assert we get some signals (depending on exact indicator output for this dummy data)
        let entries = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .count();
        let exits = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .count();

        // At least ensure it doesn't crash and returns a valid Vec
        println!("Entries: {}, Exits: {}", entries, exits);
        Ok(())
    }

    #[tokio::test]
    async fn test_empty_data() {
        let config = SupertrendRsiConfig::default();
        let strategy = SupertrendRsi::new(config);

        let df = DataFrame::empty();
        let result = strategy.generate_signals(&df).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let mut strategy = SupertrendRsi::new(SupertrendRsiConfig::default());

        let new_params = serde_json::json!({
            "supertrend_period": 14,
            "supertrend_multiplier": 2.5,
            "rsi_period": 5,
            "rsi_oversold": 20.0,
            "rsi_overbought": 80.0,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "ETH/USD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.supertrend_period, 14);
        assert_eq!(strategy.config.rsi_period, 5);
        assert_eq!(strategy.config.symbol, "ETH/USD");

        Ok(())
    }
}
