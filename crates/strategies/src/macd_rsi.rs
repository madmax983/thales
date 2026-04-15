//! The MACD + RSI Strategy
//!
//! Combines MACD for momentum and RSI for overbought/oversold levels.
//!
use crate::indicators::{atr, macd, rsi};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

/// A trend-following momentum strategy combining MACD and RSI.
///
/// It enters long when the MACD Line crosses above the Signal Line and the RSI is below a certain threshold.
/// It exits long when the MACD Line crosses below the Signal Line or the RSI exceeds a threshold.
pub struct MacdRsiTrend {
    config: MacdRsiTrendConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MacdRsiTrendConfig {
    pub macd_fast_period: usize,
    pub macd_slow_period: usize,
    pub macd_signal_period: usize,
    pub rsi_period: usize,
    pub rsi_buy_threshold: f64,
    pub rsi_sell_threshold: f64,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}

impl StrategyConfig for MacdRsiTrendConfig {}

impl MacdRsiTrend {
    pub fn new(config: MacdRsiTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for MacdRsiTrend {
    fn name(&self) -> &str {
        "MacdRsiTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        // Calculate indicators
        let (macd_line, signal_line, _hist) = macd::calculate(
            data,
            self.config.macd_fast_period,
            self.config.macd_slow_period,
            self.config.macd_signal_period,
        )?;
        let rsi_series = rsi::calculate(data, self.config.rsi_period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let macd_f64 = macd_line.f64()?;
        let signal_f64 = signal_line.f64()?;
        let rsi_f64 = rsi_series.f64()?;
        let atr_f64 = atr_series.f64()?;

        let close = data.column("close")?.f64()?;
        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let mut in_position = false;
        let mut current_stop_loss = 0.0;

        for i in 1..data.height() {
            let current_close = close.get(i);
            let prev_macd = macd_f64.get(i - 1);
            let prev_signal = signal_f64.get(i - 1);
            let curr_macd = macd_f64.get(i);
            let curr_signal = signal_f64.get(i);
            let curr_rsi = rsi_f64.get(i);
            let curr_atr = atr_f64.get(i);
            let timestamp = timestamps.get(i).unwrap_or(0);

            if let (Some(close_val), Some(pm), Some(ps), Some(cm), Some(cs), Some(cr), Some(ca)) = (
                current_close,
                prev_macd,
                prev_signal,
                curr_macd,
                curr_signal,
                curr_rsi,
                curr_atr,
            ) {
                if !in_position {
                    // Entry Condition: MACD cross above Signal AND RSI < buy_threshold
                    if pm <= ps && cm > cs && cr < self.config.rsi_buy_threshold {
                        in_position = true;
                        current_stop_loss = close_val - (ca * self.config.stop_loss_atr_mult);

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(current_stop_loss),
                            take_profit: None,
                            reason: "MACD Cross Up + RSI favorable".to_string(),
                            timestamp_ms: timestamp,
                        });
                    }
                } else {
                    // Exit Condition: MACD cross below Signal OR RSI > sell_threshold OR Stop Loss
                    let mut exit_reason = None;

                    if close_val <= current_stop_loss {
                        exit_reason = Some("Stop Loss Hit".to_string());
                    } else if pm >= ps && cm < cs {
                        exit_reason = Some("MACD Cross Down".to_string());
                    } else if cr > self.config.rsi_sell_threshold {
                        exit_reason = Some("RSI Overbought".to_string());
                    }

                    if let Some(reason) = exit_reason {
                        in_position = false;
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason,
                            timestamp_ms: timestamp,
                        });
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: MacdRsiTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        // We need enough data to calculate MACD(12,26,9), RSI(14), ATR(14).
        // Max lookback is 26+9 = 35. We need more than 35 rows.
        let mut close_vals = vec![100.0; 50];
        let mut high_vals = vec![101.0; 50];
        let mut low_vals = vec![99.0; 50];
        let timestamps: Vec<i64> = (0..50).map(|i| i as i64 * 1000).collect();

        // Create a trend down then up to force an RSI drop then a MACD cross
        for i in 20..35 {
            close_vals[i] = 100.0 - (i as f64 - 20.0); // down to 85
            high_vals[i] = close_vals[i] + 1.0;
            low_vals[i] = close_vals[i] - 1.0;
        }
        for i in 35..50 {
            close_vals[i] = 85.0 + (i as f64 - 35.0) * 2.0; // up to 115
            high_vals[i] = close_vals[i] + 1.0;
            low_vals[i] = close_vals[i] - 1.0;
        }

        df!(
            "timestamp_unix_ms" => timestamps,
            "open" => close_vals.clone(),
            "high" => high_vals,
            "low" => low_vals,
            "close" => close_vals,
            "volume" => vec![1000.0; 50]
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_macd_rsi_trend_empty_data() -> Result<()> {
        let config = MacdRsiTrendConfig {
            macd_fast_period: 12,
            macd_slow_period: 26,
            macd_signal_period: 9,
            rsi_period: 14,
            rsi_buy_threshold: 70.0,
            rsi_sell_threshold: 70.0,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };

        let strategy = MacdRsiTrend::new(config);
        let empty_df = DataFrame::empty();
        let signals = strategy.generate_signals(&empty_df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_macd_rsi_trend_signal_generation() -> Result<()> {
        let config = MacdRsiTrendConfig {
            macd_fast_period: 5,
            macd_slow_period: 10,
            macd_signal_period: 3,
            rsi_period: 5,
            rsi_buy_threshold: 100.0, // High enough to allow buy
            rsi_sell_threshold: 70.0,
            atr_period: 5,
            stop_loss_atr_mult: 1.0,
            symbol: "TEST".to_string(),
        };

        let strategy = MacdRsiTrend::new(config);
        let df = create_test_data();
        let signals = strategy.generate_signals(&df).await?;

        // Expect at least one entry signal because we set rsi_buy_threshold to 100
        // and forced a MACD crossover by dipping and rising price.
        assert!(!signals.is_empty(), "Should generate signals");

        let entry_signal = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry_signal.is_some(), "Should have an entry signal");
        if let Some(s) = entry_signal {
            assert_eq!(s.side, "buy");
            assert!(s.stop_loss.is_some());
        }

        // Also check if an exit is generated either by stop loss or overbought/macd
        let _exit_signal = signals.iter().find(|s| s.signal_type == SignalType::Exit);
        // It might not trigger exit within the 50 bars depending on exactly how fast RSI rises,
        // but we verify no crash and correct structure.

        Ok(())
    }

    #[tokio::test]
    async fn test_macd_rsi_trend_update_params() -> Result<()> {
        let mut strategy = MacdRsiTrend::new(MacdRsiTrendConfig {
            macd_fast_period: 12,
            macd_slow_period: 26,
            macd_signal_period: 9,
            rsi_period: 14,
            rsi_buy_threshold: 70.0,
            rsi_sell_threshold: 70.0,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "macd_fast_period": 5,
            "macd_slow_period": 10,
            "macd_signal_period": 3,
            "rsi_period": 5,
            "rsi_buy_threshold": 30.0,
            "rsi_sell_threshold": 80.0,
            "atr_period": 5,
            "stop_loss_atr_mult": 1.5,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.macd_fast_period, 5);
        assert_eq!(strategy.config.rsi_buy_threshold, 30.0);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
