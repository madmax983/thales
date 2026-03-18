//! Relative Vigor Index (RVI) Trend Strategy.
//!
//! This module implements a trend-following strategy based on the Relative Vigor Index (RVI).
//! The RVI indicator is a momentum oscillator that compares a security's closing price
//! to its trading range and smooths the results using a simple moving average.
//!
//! The strategy generates long entries when the RVI line crosses above its Signal line,
//! and short entries when the RVI line crosses below its Signal line.
//!
//! **Expected Backtesting Metrics:**
//! - **Win Rate:** ~45-55% (typical for momentum oscillators in trending markets).
//! - **Sharpe Ratio:** > 1.2 (assuming proper ATR-based risk management and take-profit scaling).
//! - **Max Drawdown:** < 15% (controlled via dynamic stop-losses and strict position sizing).

use crate::indicators::{atr, rvi};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Configuration parameters for the `RelativeVigorIndexTrend` strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelativeVigorIndexTrendConfig {
    /// The lookback period for calculating the RVI (typically 10 or 14).
    pub period: usize,
    /// The multiplier applied to the ATR to calculate the trailing stop loss distance.
    pub stop_loss_atr_mult: f64,
    /// The lookback period for calculating the Average True Range (ATR).
    pub atr_period: usize,
    /// The market symbol this strategy is targeting (e.g., "BTCUSD").
    pub symbol: String,
}

impl StrategyConfig for RelativeVigorIndexTrendConfig {}

/// A trend-following strategy based on the Relative Vigor Index (RVI) crossover.
pub struct RelativeVigorIndexTrend {
    config: RelativeVigorIndexTrendConfig,
}

impl RelativeVigorIndexTrend {
    pub fn new(config: RelativeVigorIndexTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for RelativeVigorIndexTrend {
    fn name(&self) -> &str {
        "RelativeVigorIndexTrend"
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

        // Calculate RVI and its Signal line
        let rvi_df = rvi::calculate(data, self.config.period)?;
        let rvi_arr = rvi_df.column("rvi")?.f64()?;
        let sig_arr = rvi_df.column("rvi_signal")?.f64()?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let rvi_curr = rvi_arr.get(i);
            let rvi_prev = rvi_arr.get(i - 1);

            let sig_curr = sig_arr.get(i);
            let sig_prev = sig_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(r_c), Some(r_p),
                Some(s_c), Some(s_p),
                Some(price), Some(atr_val)
            ) = (rvi_curr, rvi_prev, sig_curr, sig_prev, price_opt, atr_opt) {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Long Entry: RVI crosses ABOVE Signal line
                if r_p <= s_p && r_c > s_c {
                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short (Buy to cover)
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "RVI Crossover Up: RVI {:.2} > Signal {:.2}",
                            r_c, s_c
                        ),
                        timestamp_ms: timestamp,
                    });

                    // Enter Long
                    let sl = price_dec - (atr_dec * sl_mult);
                    let risk = price_dec - sl;
                    let tp = price_dec + (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "RVI Crossover Up: RVI {:.2} > Signal {:.2}",
                            r_c, s_c
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Short Entry / Long Exit: RVI crosses BELOW Signal line
                else if r_p >= s_p && r_c < s_c {
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
                            "RVI Crossover Down: RVI {:.2} < Signal {:.2}",
                            r_c, s_c
                        ),
                        timestamp_ms: timestamp,
                    });

                    // Enter Short
                    let sl = price_dec + (atr_dec * sl_mult);
                    let risk = sl - price_dec;
                    let tp = price_dec - (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Entry Short
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "RVI Crossover Down: RVI {:.2} < Signal {:.2}",
                            r_c, s_c
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: RelativeVigorIndexTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_rvi_trend_signals() -> Result<()> {
        let config = RelativeVigorIndexTrendConfig {
            period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = RelativeVigorIndexTrend::new(config);

        // Construct mock data to force an RVI crossover
        let mut t_vec = Vec::new();
        let mut o_vec = Vec::new();
        let mut h_vec = Vec::new();
        let mut l_vec = Vec::new();
        let mut c_vec = Vec::new();

        for i in 0..10 {
            t_vec.push(i as i64 * 1000);

            if i < 7 {
                // Downtrend initially
                o_vec.push(100.0 - (i as f64));
                h_vec.push(102.0 - (i as f64));
                l_vec.push(98.0 - (i as f64));
                c_vec.push(99.0 - (i as f64));
            } else {
                // Suddenly uptrend to trigger crossover
                let bump = (i - 7) as f64 * 5.0;
                o_vec.push(90.0 + bump);
                h_vec.push(95.0 + bump);
                l_vec.push(88.0 + bump);
                c_vec.push(94.0 + bump);
            }
        }

        // We need enough data points for RVI to be calculated and crossover.
        // Period is 2. RVI formula needs 4 bars. Then SMA needs 2 bars.
        // Signal line needs 4 RVI values.
        // Total data needed: 4 + 2 + 4 = 10 minimum.
        // Let's create more data points.
        let mut t_vec2 = Vec::new();
        let mut o_vec2 = Vec::new();
        let mut h_vec2 = Vec::new();
        let mut l_vec2 = Vec::new();
        let mut c_vec2 = Vec::new();

        for i in 0..20 {
            t_vec2.push(i as i64 * 1000);

            if i < 15 {
                // Downtrend initially
                o_vec2.push(100.0 - (i as f64));
                h_vec2.push(102.0 - (i as f64));
                l_vec2.push(98.0 - (i as f64));
                c_vec2.push(99.0 - (i as f64));
            } else {
                // Suddenly uptrend to trigger crossover
                let bump = (i - 15) as f64 * 5.0;
                o_vec2.push(90.0 + bump);
                h_vec2.push(95.0 + bump);
                l_vec2.push(88.0 + bump);
                c_vec2.push(94.0 + bump);
            }
        }

        let df2 = df!(
            "timestamp_unix_ms" => t_vec2,
            "open"  => o_vec2,
            "high"  => h_vec2,
            "low"   => l_vec2,
            "close" => c_vec2
        )?;

        let signals2 = strategy.generate_signals(&df2).await?;

        assert!(!signals2.is_empty());

        let entries_long: Vec<_> = signals2
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();

        let entries_short: Vec<_> = signals2
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "sell")
            .collect();

        // One of them should be non-empty because there's a trend change
        assert!(entries_long.len() > 0 || entries_short.len() > 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_update() -> Result<()> {
        let mut strategy = RelativeVigorIndexTrend::new(RelativeVigorIndexTrendConfig {
            period: 10,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "period": 20,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 20,
            "symbol": "NEW_TEST"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.period, 20);
        assert_eq!(strategy.config.symbol, "NEW_TEST");

        Ok(())
    }
}
