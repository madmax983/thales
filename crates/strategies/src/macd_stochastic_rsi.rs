use crate::indicators::{atr, macd, stoch_rsi};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

/// A momentum strategy combining MACD and Stochastic RSI.
///
/// It enters long when the MACD Line crosses above the Signal Line and the Stochastic RSI is oversold.
/// It exits long when the MACD Line crosses below the Signal Line or the Stochastic RSI becomes overbought.
pub struct MacdStochasticRsi {
    config: MacdStochasticRsiConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MacdStochasticRsiConfig {
    pub macd_fast_period: usize,
    pub macd_slow_period: usize,
    pub macd_signal_period: usize,
    pub rsi_period: usize,
    pub stoch_period: usize,
    pub k_period: usize,
    pub d_period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}

impl StrategyConfig for MacdStochasticRsiConfig {}

impl MacdStochasticRsi {
    pub fn new(config: MacdStochasticRsiConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for MacdStochasticRsi {
    fn name(&self) -> &str {
        "MacdStochasticRsi"
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

        let (k_series, d_series) = stoch_rsi::calculate(
            data,
            self.config.rsi_period,
            self.config.stoch_period,
            self.config.k_period,
            self.config.d_period,
        )?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let macd_f64 = macd_line.f64()?;
        let signal_f64 = signal_line.f64()?;
        let k_f64 = k_series.f64()?;
        let d_f64 = d_series.f64()?;
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
            let curr_k = k_f64.get(i);
            let curr_d = d_f64.get(i);
            let curr_atr = atr_f64.get(i);
            let timestamp = timestamps.get(i).unwrap_or(0);

            if let (
                Some(close_val),
                Some(pm),
                Some(ps),
                Some(cm),
                Some(cs),
                Some(ck),
                Some(cd),
                Some(ca),
            ) = (
                current_close,
                prev_macd,
                prev_signal,
                curr_macd,
                curr_signal,
                curr_k,
                curr_d,
                curr_atr,
            ) {
                if !in_position {
                    // Entry Condition: MACD cross above Signal AND StochRSI K < oversold_threshold
                    if pm <= ps && cm > cs && ck < self.config.oversold_threshold {
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
                            reason: "MACD Cross Up + StochRSI oversold".to_string(),
                            timestamp_ms: timestamp,
                        });
                    }
                } else {
                    // Exit Condition: MACD cross below Signal OR StochRSI K > overbought_threshold OR Stop Loss
                    let mut exit_reason = None;

                    if close_val <= current_stop_loss {
                        exit_reason = Some("Stop Loss Hit".to_string());
                    } else if pm >= ps && cm < cs {
                        exit_reason = Some("MACD Cross Down".to_string());
                    } else if ck > self.config.overbought_threshold {
                        exit_reason = Some("StochRSI Overbought".to_string());
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
        let new_config: MacdStochasticRsiConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        let mut close_vals = vec![100.0; 100];
        let mut high_vals = vec![101.0; 100];
        let mut low_vals = vec![99.0; 100];
        let timestamps: Vec<i64> = (0..100).map(|i| i as i64 * 1000).collect();

        // Create a down trend followed by up trend
        for i in 20..50 {
            close_vals[i] = 100.0 - (i as f64 - 20.0);
            high_vals[i] = close_vals[i] + 1.0;
            low_vals[i] = close_vals[i] - 1.0;
        }
        for i in 50..80 {
            close_vals[i] = 70.0 + (i as f64 - 50.0) * 2.0;
            high_vals[i] = close_vals[i] + 1.0;
            low_vals[i] = close_vals[i] - 1.0;
        }

        df!(
            "timestamp_unix_ms" => timestamps,
            "open" => close_vals.clone(),
            "high" => high_vals,
            "low" => low_vals,
            "close" => close_vals,
            "volume" => vec![1000.0; 100]
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_macd_stoch_rsi_empty_data() -> Result<()> {
        let config = MacdStochasticRsiConfig {
            macd_fast_period: 12,
            macd_slow_period: 26,
            macd_signal_period: 9,
            rsi_period: 14,
            stoch_period: 14,
            k_period: 3,
            d_period: 3,
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };

        let strategy = MacdStochasticRsi::new(config);
        let empty_df = DataFrame::empty();
        let signals = strategy.generate_signals(&empty_df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_macd_stoch_rsi_signal_generation() -> Result<()> {
        let config = MacdStochasticRsiConfig {
            macd_fast_period: 5,
            macd_slow_period: 10,
            macd_signal_period: 3,
            rsi_period: 5,
            stoch_period: 5,
            k_period: 3,
            d_period: 3,
            oversold_threshold: 100.0, // High enough to allow buy
            overbought_threshold: 80.0,
            atr_period: 5,
            stop_loss_atr_mult: 1.0,
            symbol: "TEST".to_string(),
        };

        let strategy = MacdStochasticRsi::new(config);
        let df = create_test_data();
        let signals = strategy.generate_signals(&df).await?;

        assert!(!signals.is_empty(), "Should generate signals");

        let entry_signal = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry_signal.is_some(), "Should have an entry signal");
        if let Some(s) = entry_signal {
            assert_eq!(s.side, "buy");
            assert!(s.stop_loss.is_some());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_macd_stoch_rsi_update_params() -> Result<()> {
        let mut strategy = MacdStochasticRsi::new(MacdStochasticRsiConfig {
            macd_fast_period: 12,
            macd_slow_period: 26,
            macd_signal_period: 9,
            rsi_period: 14,
            stoch_period: 14,
            k_period: 3,
            d_period: 3,
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "macd_fast_period": 5,
            "macd_slow_period": 10,
            "macd_signal_period": 3,
            "rsi_period": 5,
            "stoch_period": 5,
            "k_period": 3,
            "d_period": 3,
            "oversold_threshold": 30.0,
            "overbought_threshold": 80.0,
            "atr_period": 5,
            "stop_loss_atr_mult": 1.5,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.macd_fast_period, 5);
        assert_eq!(strategy.config.oversold_threshold, 30.0);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
