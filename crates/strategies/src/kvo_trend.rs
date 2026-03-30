//! Klinger Volume Oscillator (KVO) Trend Following Strategy.
//!
//! This strategy uses the Klinger Volume Oscillator to identify long-term trends
//! of money flow. It enters long when the KVO crosses above its signal line, and
//! short when it crosses below its signal line.

use crate::indicators::{atr, kvo};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Configuration for the [`KvoTrendFollowing`] strategy.
///
/// Defines the periods for the KVO calculations, as well as risk management parameters.
///
/// # Examples
///
/// ```rust
/// use strategies::kvo_trend::KvoTrendFollowingConfig;
///
/// let config = KvoTrendFollowingConfig {
///     kvo_fast_period: 34,
///     kvo_slow_period: 55,
///     kvo_signal_period: 13,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     symbol: "BTCUSD".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvoTrendFollowingConfig {
    pub kvo_fast_period: usize,
    pub kvo_slow_period: usize,
    pub kvo_signal_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for KvoTrendFollowingConfig {
    fn default() -> Self {
        Self {
            kvo_fast_period: 34,
            kvo_slow_period: 55,
            kvo_signal_period: 13,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        }
    }
}

impl StrategyConfig for KvoTrendFollowingConfig {}

/// The KVO Trend Following strategy implementation.
///
/// Generates signals based on the crossover of the KVO and its signal line.
pub struct KvoTrendFollowing {
    config: KvoTrendFollowingConfig,
}

impl KvoTrendFollowing {
    pub fn new(config: KvoTrendFollowingConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for KvoTrendFollowing {
    fn name(&self) -> &str {
        "KvoTrendFollowing"
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

        // Calculate KVO and Signal
        let kvo_df = kvo::calculate(
            data,
            self.config.kvo_fast_period,
            self.config.kvo_slow_period,
            self.config.kvo_signal_period,
        )?;

        let kvo_arr = kvo_df.column("kvo")?.f64()?;
        let kvo_sig_arr = kvo_df.column("kvo_signal")?.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));
        let two_dec = Decimal::new(2, 0);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);

            let k_curr_opt = kvo_arr.get(i).and_then(Decimal::from_f64_retain);
            let s_curr_opt = kvo_sig_arr.get(i).and_then(Decimal::from_f64_retain);
            let k_prev_opt = kvo_arr.get(i - 1).and_then(Decimal::from_f64_retain);
            let s_prev_opt = kvo_sig_arr.get(i - 1).and_then(Decimal::from_f64_retain);

            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (
                Some(kc),
                Some(sc),
                Some(kp),
                Some(sp),
                Some(price),
                Some(atr_val),
            ) = (
                k_curr_opt,
                s_curr_opt,
                k_prev_opt,
                s_prev_opt,
                price_opt,
                atr_opt,
            ) {
                // Bullish Crossover (Entry Long)
                if kp <= sp && kc > sc {
                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bullish Crossover: KVO {} > Signal {}",
                            kc.round_dp(2),
                            sc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });

                    let sl = price - (atr_val * atr_mult_dec);
                    let risk = price - sl;
                    let tp = price + (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "Bullish Crossover: KVO {} > Signal {}",
                            kc.round_dp(2),
                            sc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Bearish Crossover (Entry Short)
                else if kp >= sp && kc < sc {
                    // Exit any Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bearish Crossover: KVO {} < Signal {}",
                            kc.round_dp(2),
                            sc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });

                    let sl = price + (atr_val * atr_mult_dec);
                    let risk = sl - price;
                    let tp = price - (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Entry Short
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "Bearish Crossover: KVO {} < Signal {}",
                            kc.round_dp(2),
                            sc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: KvoTrendFollowingConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_kvo_trend_signals() -> Result<()> {
        let config = KvoTrendFollowingConfig {
            kvo_fast_period: 2,
            kvo_slow_period: 4,
            kvo_signal_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = KvoTrendFollowing::new(config);

        // Generate synthetic data
        let mut closes = Vec::new();
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut volumes = Vec::new();
        let mut timestamps = Vec::new();

        for i in 0..20 {
            timestamps.push(i as i64 * 1000);

            if i < 10 {
                // Downtrend initially
                closes.push(100.0 - (i as f64));
                highs.push(102.0 - (i as f64));
                lows.push(98.0 - (i as f64));
                volumes.push(100.0);
            } else {
                // Suddenly uptrend to trigger crossover
                let bump = (i - 10) as f64 * 5.0;
                closes.push(90.0 + bump);
                highs.push(95.0 + bump);
                lows.push(88.0 + bump);
                volumes.push(500.0);
            }
        }

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows,
            "volume" => volumes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect at least one crossover signal
        assert!(!signals.is_empty());

        let entries_long: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();

        let entries_short: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "sell")
            .collect();

        // At least one side should be triggered
        assert!(!entries_long.is_empty() || !entries_short.is_empty());

        Ok(())
    }
}
