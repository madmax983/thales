//! The ADX Momentum Strategy
//!
//! Uses the ADX indicator to trigger trades when trend strength reaches a certain threshold.
//!
use crate::indicators::{adx, atr};
use crate::strategy::{Signal, SignalType, Strategy, StrategyType};
use anyhow::{Context, Result};
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

pub struct AdxMomentum {
    config: AdxMomentumConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdxMomentumConfig {
    pub adx_period: usize,
    pub adx_threshold: f64,
    pub di_period: usize, // Usually same as adx_period, but can be distinct if library supports it.
    // Our adx::calculate uses one period for both.
    // So we will use adx_period for calculation and ignore this or assume equality.
    // To avoid confusion, let's use adx_period for the indicator call.
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl AdxMomentum {
    pub fn new(config: AdxMomentumConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for AdxMomentum {
    fn name(&self) -> &str {
        "AdxMomentum"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let (adx_s, p_di_s, m_di_s) =
            adx::calculate(data, self.config.adx_period).context("Failed to calculate ADX")?;

        let atr_s =
            atr::calculate(data, self.config.atr_period).context("Failed to calculate ATR")?;

        let close_s = data.column("close")?.f64()?;
        let time_s = data.column("timestamp_unix_ms")?.i64()?;

        // Convert Series to Vec for iteration
        let adx_v: Vec<Option<f64>> = adx_s.f64()?.into_iter().collect();
        let p_di_v: Vec<Option<f64>> = p_di_s.f64()?.into_iter().collect();
        let m_di_v: Vec<Option<f64>> = m_di_s.f64()?.into_iter().collect();
        let atr_v: Vec<Option<f64>> = atr_s.f64()?.into_iter().collect();

        let mut signals = Vec::new();
        let len = data.height();

        // State to track if we are currently in a trade (virtual) to avoid re-signaling or to handle exits properly?
        // The generate_signals interface usually produces signals based on conditions at each bar.
        // It's stateless in terms of "held positions" unless we simulate it.
        // But for Entry, we want "Condition became true".

        for i in 1..len {
            if let (
                Some(adx_val),
                Some(p_di),
                Some(m_di),
                Some(prev_adx),
                Some(prev_p_di),
                Some(prev_m_di),
                Some(close),
                Some(atr),
            ) = (
                adx_v[i],
                p_di_v[i],
                m_di_v[i],
                adx_v[i - 1],
                p_di_v[i - 1],
                m_di_v[i - 1],
                close_s.get(i),
                atr_v[i],
            ) {
                let timestamp = time_s.get(i).unwrap_or(0);

                let is_bullish = p_di > m_di;
                let is_strong = adx_val > self.config.adx_threshold;

                let was_bullish = prev_p_di > prev_m_di;
                let was_strong = prev_adx > self.config.adx_threshold;

                let condition_met = is_bullish && is_strong;
                let prev_condition_met = was_bullish && was_strong;

                // Entry Signal
                if condition_met && !prev_condition_met {
                    let stop_loss = close - (atr * self.config.stop_loss_atr_mult);
                    // Take profit? Trend following usually lets it run.
                    // But we can set a wide TP or let the exit logic handle it.
                    // Let's set a 4R TP as a hint, or None.
                    // existing strategies often don't set TP or set it via risk ratio.
                    // I'll set it to 3 * Risk.
                    let take_profit = close + (atr * self.config.stop_loss_atr_mult * 3.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(), // Will be overridden by risk manager
                        confidence: 0.8,              // High confidence for trend
                        stop_loss: Some(stop_loss),
                        take_profit: Some(take_profit),
                        reason: format!(
                            "ADX ({:.2}) > {:.0} and +DI > -DI",
                            adx_val, self.config.adx_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Exit Signal
                // 1. Trend Reversal: +DI crosses below -DI
                if !is_bullish && was_bullish {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.6,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Trend Reversal (+DI < -DI)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // 2. Trend Weakening: ADX drops below threshold
                // Only if we were strong before.
                if !is_strong && was_strong {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.6,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Trend Weakening (ADX {:.2} < {:.0})",
                            adx_val, self.config.adx_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: AdxMomentumConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adx_momentum_signals() -> Result<()> {
        // Create synthetic data
        // 1. Ranging (ADX < 25)
        // 2. Strong Uptrend (ADX > 25, +DI > -DI) -> Entry
        // 3. Reversal -> Exit

        // We need enough data for ADX period (14) + smoothing. Approx 28 bars.
        let mut bars = Vec::new();
        let start_ts = 100000;

        // 30 bars of ranging/noise (flat price)
        for i in 0..30 {
            let p = 100.0 + (i % 2) as f64; // 100, 101, 100...
            bars.push((p, p + 1.0, p - 1.0));
        }

        // 20 bars of strong uptrend
        // Price increases by 2 every bar
        for i in 30..50 {
            let p = 100.0 + (i - 30) as f64 * 2.0;
            bars.push((p, p + 1.0, p - 1.0));
        }

        // 10 bars of sharp drop (Reversal)
        for i in 50..60 {
            let p = 140.0 - (i - 50) as f64 * 3.0;
            bars.push((p, p + 1.0, p - 1.0));
        }

        let closes: Vec<f64> = bars.iter().map(|b| b.0).collect();
        let highs: Vec<f64> = bars.iter().map(|b| b.1).collect();
        let lows: Vec<f64> = bars.iter().map(|b| b.2).collect();
        let times: Vec<i64> = (0..bars.len())
            .map(|i| start_ts + i as i64 * 60000)
            .collect();

        let df = df!(
            "close" => closes,
            "high" => highs,
            "low" => lows,
            "timestamp_unix_ms" => times
        )?;

        let config = AdxMomentumConfig {
            adx_period: 14,
            adx_threshold: 20.0, // Lower threshold for test to ensure trigger
            di_period: 14,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = AdxMomentum::new(config);
        let signals = strategy.generate_signals(&df).await?;

        // We expect:
        // 1. Entry signal when trend starts (somewhere after index 30+14?)
        // 2. Exit signal when trend reverses (index 50+)

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        assert!(!entries.is_empty(), "Should generate entry signals");
        assert!(!exits.is_empty(), "Should generate exit signals");

        // Verify Entry logic
        let entry = entries[0];
        assert_eq!(entry.side, "buy");
        assert!(entry.reason.contains("ADX"));

        // Verify Exit logic
        let exit = exits.last().unwrap();
        assert_eq!(exit.side, "sell");
        // Could be Trend Reversal or Weakening depending on which happened first/last

        Ok(())
    }
}
