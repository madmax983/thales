//! The Know Sure Thing (KST) Strategy
//!
//! Uses the KST momentum oscillator to capture major trends.
//!
use crate::strategy::{Signal, SignalType, Strategy, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::indicators::{atr, kst};

/// Know Sure Thing (KST) Trend Following Strategy
///
/// This strategy uses the KST oscillator and its signal line to identify trends.
/// Entry signals are generated when the KST line crosses above the signal line (bullish)
/// or below the signal line (bearish).
pub struct KstTrend {
    config: KstTrendConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KstTrendConfig {
    pub roc_periods: [usize; 4],
    pub sma_periods: [usize; 4],
    pub signal_period: usize,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub max_position_size: f64,
    pub symbol: String,
}

impl Default for KstTrendConfig {
    fn default() -> Self {
        Self {
            roc_periods: [10, 15, 20, 30],
            sma_periods: [10, 10, 10, 15],
            signal_period: 9,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            max_position_size: 1.0,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl KstTrend {
    pub fn new(config: KstTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for KstTrend {
    fn name(&self) -> &str {
        "KstTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            anyhow::bail!("Data cannot be empty");
        }

        let mut signals = Vec::new();

        // Calculate KST
        let (kst_series, signal_series) = kst::calculate(
            data,
            self.config.roc_periods,
            self.config.sma_periods,
            self.config.signal_period,
        )?;

        // Calculate ATR for stop loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let closes = data.column("close")?.f64()?;
        let kst_vals = kst_series.f64()?;
        let sig_vals = signal_series.f64()?;
        let atr_vals = atr_series.f64()?;

        let ts_col = data
            .column("timestamp")
            .ok()
            .and_then(|c| c.datetime().ok());

        // Need at least 2 points to check crossover
        for i in 1..closes.len() {
            let close = closes.get(i);
            let prev_kst = kst_vals.get(i - 1);
            let curr_kst = kst_vals.get(i);
            let prev_sig = sig_vals.get(i - 1);
            let curr_sig = sig_vals.get(i);
            let atr = atr_vals.get(i);

            let ts = if let Some(col) = ts_col {
                col.get(i).unwrap_or(Utc::now().timestamp_millis())
            } else {
                Utc::now().timestamp_millis()
            };

            if let (
                Some(close),
                Some(prev_kst),
                Some(curr_kst),
                Some(prev_sig),
                Some(curr_sig),
                Some(atr),
            ) = (close, prev_kst, curr_kst, prev_sig, curr_sig, atr)
            {
                // Buy Signal: KST crosses ABOVE Signal line
                if prev_kst <= prev_sig && curr_kst > curr_sig {
                    let stop_loss = close - (atr * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: self.config.max_position_size.to_string(), // Limited by max_position_size config
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: format!(
                            "KST ({:.2}) crossed above Signal ({:.2})",
                            curr_kst, curr_sig
                        ),
                        timestamp_ms: ts,
                    });
                }
                // Sell Signal: KST crosses BELOW Signal line
                else if prev_kst >= prev_sig && curr_kst < curr_sig {
                    let stop_loss = close + (atr * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: self.config.max_position_size.to_string(), // Limited by max_position_size config
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: format!(
                            "KST ({:.2}) crossed below Signal ({:.2})",
                            curr_kst, curr_sig
                        ),
                        timestamp_ms: ts,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: KstTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_kst_trend_buy_signal() -> Result<()> {
        // Need to construct data where KST crosses above Signal
        // KST responds to momentum. An accelerating trend upward should trigger a buy.
        // We'll use mocked data that stays flat, then accelerates up.
        let mut closes = Vec::new();
        let mut highs = Vec::new();
        let mut lows = Vec::new();

        let mut price = 100.0;
        for i in 0..100 {
            if i > 50 {
                // Accelerating uptrend
                price += (i - 50) as f64 * 0.5;
            } else {
                // Flat/chop
                price += if i % 2 == 0 { 1.0 } else { -1.0 };
            }
            closes.push(price);
            highs.push(price + 2.0);
            lows.push(price - 2.0);
        }

        let df = df!(
            "close" => &closes,
            "high" => &highs,
            "low" => &lows,
        )?;

        let config = KstTrendConfig {
            roc_periods: [2, 3, 4, 5],
            sma_periods: [2, 2, 2, 2],
            signal_period: 3,
            atr_period: 2,
            stop_loss_atr_mult: 1.0,
            max_position_size: 1.0,
            symbol: "TEST".to_string(),
        };

        let strategy = KstTrend::new(config);
        let signals = strategy.generate_signals(&df).await?;

        // We expect at least one buy signal as momentum turned positive
        assert!(!signals.is_empty());

        // Find a buy signal
        let buy_signals: Vec<_> = signals.iter().filter(|s| s.side == "buy").collect();
        assert!(!buy_signals.is_empty(), "Expected at least one buy signal");

        let signal = buy_signals.last().unwrap();
        assert_eq!(signal.symbol, "TEST");
        assert!(signal.reason.contains("crossed above Signal"));

        Ok(())
    }

    #[tokio::test]
    async fn test_kst_trend_sell_signal() -> Result<()> {
        // Construct data where momentum turns negative
        let mut closes = Vec::new();
        let mut highs = Vec::new();
        let mut lows = Vec::new();

        let mut price = 200.0;
        for i in 0..100 {
            if i > 50 {
                // Accelerating downtrend
                price -= (i - 50) as f64 * 0.5;
            } else {
                // Flat/chop
                price += if i % 2 == 0 { 1.0 } else { -1.0 };
            }
            closes.push(price);
            highs.push(price + 2.0);
            lows.push(price - 2.0);
        }

        let df = df!(
            "close" => &closes,
            "high" => &highs,
            "low" => &lows,
        )?;

        let config = KstTrendConfig {
            roc_periods: [2, 3, 4, 5],
            sma_periods: [2, 2, 2, 2],
            signal_period: 3,
            atr_period: 2,
            stop_loss_atr_mult: 1.0,
            max_position_size: 1.0,
            symbol: "TEST".to_string(),
        };

        let strategy = KstTrend::new(config);
        let signals = strategy.generate_signals(&df).await?;

        // Expect at least one sell signal
        assert!(!signals.is_empty());

        let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == "sell").collect();
        assert!(
            !sell_signals.is_empty(),
            "Expected at least one sell signal"
        );

        let signal = sell_signals.last().unwrap();
        assert_eq!(signal.symbol, "TEST");
        assert!(signal.reason.contains("crossed below Signal"));

        Ok(())
    }

    #[tokio::test]
    async fn test_kst_trend_edge_cases() -> Result<()> {
        let strategy = KstTrend::new(KstTrendConfig::default());

        // Empty data
        let df_empty = DataFrame::default();
        let result = strategy.generate_signals(&df_empty).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Data cannot be empty");

        // Not enough data for KST (returns empty signals list, not error)
        let df_small = df!(
            "close" => &[100.0, 101.0],
            "high" => &[102.0, 102.0],
            "low" => &[99.0, 99.0],
        )?;
        let result = strategy.generate_signals(&df_small).await?;
        assert!(result.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let mut strategy = KstTrend::new(KstTrendConfig::default());

        let new_params = serde_json::json!({
            "roc_periods": [5, 10, 15, 20],
            "sma_periods": [5, 5, 5, 10],
            "signal_period": 5,
            "atr_period": 10,
            "stop_loss_atr_mult": 1.5,
            "max_position_size": 2.0,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.symbol, "BTCUSD");
        assert_eq!(strategy.config.roc_periods, [5, 10, 15, 20]);
        assert_eq!(strategy.config.atr_period, 10);

        Ok(())
    }
}
