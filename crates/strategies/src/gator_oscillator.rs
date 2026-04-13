//! Gator Oscillator Strategy
//!
//! A trend-following oscillator based on Bill Williams' Gator Oscillator. It expands the Alligator
//! indicator to visualize the widening and narrowing of the Jaw, Teeth, and Lips lines, indicating
//! trend strength and phase (sleeping, awakening, eating, sated).

use crate::indicators::{atr, gator};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::{Context, Result};
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;

/// Configuration for the Gator Oscillator Strategy
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GatorOscillatorConfig {
    pub jaw_period: usize,
    pub jaw_shift: usize,
    pub teeth_period: usize,
    pub teeth_shift: usize,
    pub lips_period: usize,
    pub lips_shift: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub max_position_size: f64,
    pub symbol: String,
}

impl Default for GatorOscillatorConfig {
    fn default() -> Self {
        Self {
            jaw_period: 13,
            jaw_shift: 8,
            teeth_period: 8,
            teeth_shift: 5,
            lips_period: 5,
            lips_shift: 3,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            max_position_size: 100.0,
            symbol: "XXBTZUSD".to_string(),
        }
    }
}

impl StrategyConfig for GatorOscillatorConfig {}

/// Gator Oscillator Strategy implementation
pub struct GatorOscillator {
    config: GatorOscillatorConfig,
}

impl GatorOscillator {
    pub fn new(config: GatorOscillatorConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for GatorOscillator {
    fn name(&self) -> &str {
        "GatorOscillator"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let mut signals = Vec::new();

        let req_len = self.config.jaw_period + self.config.jaw_shift;
        if data.height() < req_len {
            return Ok(signals);
        }

        let close_series = data
            .column("close")
            .context("DataFrame must contain 'close' column")?
            .f64()
            .context("Close column must be numeric")?;

        let timestamp_series = data
            .column("timestamp_unix_ms")
            .context("DataFrame must contain 'timestamp_unix_ms' column")?
            .i64()
            .context("Timestamp column must be numeric")?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_f64 = atr_series.f64()?;

        let (upper_series, lower_series) = gator::calculate(
            data,
            self.config.jaw_period,
            self.config.jaw_shift,
            self.config.teeth_period,
            self.config.teeth_shift,
            self.config.lips_period,
            self.config.lips_shift,
        )?;

        let upper_f64 = upper_series.f64()?;
        let lower_f64 = lower_series.f64()?;

        let mut was_expanding = false;
        let mut was_contracting = false;

        for i in 1..upper_f64.len() {
            let curr_upper = upper_f64.get(i);
            let prev_upper = upper_f64.get(i - 1);

            let curr_lower = lower_f64.get(i);
            let prev_lower = lower_f64.get(i - 1);

            let curr_close = close_series.get(i);
            let curr_ts = timestamp_series.get(i);
            let curr_atr = atr_f64.get(i);

            if let (
                Some(c_up),
                Some(p_up),
                Some(c_low),
                Some(p_low),
                Some(close_price),
                Some(ts),
                Some(atr_val)
            ) = (
                curr_upper,
                prev_upper,
                curr_lower,
                prev_lower,
                curr_close,
                curr_ts,
                curr_atr,
            ) {
                // Upper is positive, so expanding means c_up > p_up.
                // Lower is negative, so expanding (abs increasing) means c_low < p_low.
                let is_expanding = c_up > p_up && c_low < p_low;

                // Contracting means abs decreasing.
                // c_up < p_up and c_low > p_low
                let is_contracting = c_up < p_up && c_low > p_low;

                let sl_dist = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO)
                    * Decimal::from_f64_retain(self.config.stop_loss_atr_mult)
                        .unwrap_or(Decimal::ZERO);

                if is_expanding && !was_expanding {
                    // Long Entry
                    let sl_price = (Decimal::from_f64_retain(close_price).unwrap_or(Decimal::ZERO)
                        - sl_dist)
                        .to_f64()
                        .unwrap_or(0.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl_price),
                        take_profit: None,
                        reason: "Gator histograms expanding".to_string(),
                        timestamp_ms: ts,
                    });

                    // Short Exit
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // exit short implies buying
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Gator histograms expanding (Short Exit)".to_string(),
                        timestamp_ms: ts,
                    });
                } else if is_contracting && !was_contracting {
                    // Short Entry
                    let sl_price = (Decimal::from_f64_retain(close_price).unwrap_or(Decimal::ZERO)
                        + sl_dist)
                        .to_f64()
                        .unwrap_or(0.0);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl_price),
                        take_profit: None,
                        reason: "Gator histograms contracting".to_string(),
                        timestamp_ms: ts,
                    });

                    // Long Exit
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // exit long implies selling
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Gator histograms contracting (Long Exit)".to_string(),
                        timestamp_ms: ts,
                    });
                }

                was_expanding = is_expanding;
                was_contracting = is_contracting;
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: GatorOscillatorConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_empty_data() {
        let df = DataFrame::default();
        let config = GatorOscillatorConfig::default();
        let strategy = GatorOscillator::new(config);

        let result = strategy.generate_signals(&df).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let config = GatorOscillatorConfig::default();
        let mut strategy = GatorOscillator::new(config);

        let new_params = serde_json::json!({
            "jaw_period": 20,
            "jaw_shift": 10,
            "teeth_period": 10,
            "teeth_shift": 6,
            "lips_period": 6,
            "lips_shift": 4,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 14,
            "max_position_size": 200.0,
            "symbol": "ETHUSD"
        });

        strategy.update_params(new_params).await.unwrap();
        assert_eq!(strategy.config.jaw_period, 20);
        assert_eq!(strategy.config.stop_loss_atr_mult, 1.5);
        assert_eq!(strategy.config.max_position_size, 200.0);
        assert_eq!(strategy.config.symbol, "ETHUSD");

        let bad_params = serde_json::json!({
            "jaw_period": 0, // Invalid
            "jaw_shift": 10,
            "teeth_period": 10,
            "teeth_shift": 6,
            "lips_period": 6,
            "lips_shift": 4,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 14,
            "max_position_size": 200.0,
            "symbol": "ETHUSD"
        });

        // Deserialization doesn't fail on 0 because usize can be 0, but we can test logic that relies on > 0
        let new_config: GatorOscillatorConfig = serde_json::from_value(bad_params).unwrap();
        let bad_strategy = GatorOscillator::new(new_config);
        let df = df!("high" => &[10.0], "low" => &[8.0], "close" => &[9.0], "timestamp_unix_ms" => &[1000i64]).unwrap();
        let _res = bad_strategy.generate_signals(&df).await;
        // The indicators check for period=0 and will return err, though here we might return empty due to short data.
    }

    #[tokio::test]
    async fn test_expanding_signals() {
        // We will create a small setup where current is expanding compared to previous.
        // Jaw, Teeth, Lips SMMA calculations are complex, so we will generate a long trend
        // to naturally produce an expansion at the end.
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut closes = Vec::new();
        let mut timestamps = Vec::new();

        // 50 periods of slow uptrend, then 10 periods of steep uptrend
        for i in 0..50 {
            highs.push(100.0 + (i as f64) * 0.1);
            lows.push(95.0 + (i as f64) * 0.1);
            closes.push(97.0 + (i as f64) * 0.1);
            timestamps.push((i as i64) * 1000);
        }

        for i in 50..60 {
            highs.push(105.0 + ((i - 50) as f64) * 5.0);
            lows.push(100.0 + ((i - 50) as f64) * 5.0);
            closes.push(102.0 + ((i - 50) as f64) * 5.0);
            timestamps.push((i as i64) * 1000);
        }

        let data = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes,
            "timestamp_unix_ms" => timestamps,
        ).unwrap();

        let config = GatorOscillatorConfig::default();
        let strategy = GatorOscillator::new(config);
        let signals = strategy.generate_signals(&data).await.unwrap();

        // Should detect expansion
        let entry_signals: Vec<_> = signals.iter().filter(|s| s.signal_type == SignalType::Entry).collect();
        assert!(!entry_signals.is_empty(), "Expected entry signal due to expansion");

        if let Some(signal) = entry_signals.first() {
            assert_eq!(signal.side, "buy");
            assert_eq!(signal.reason, "Gator histograms expanding");
        }
    }

    #[tokio::test]
    async fn test_contracting_signals() {
        // Trend up, then flat
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut closes = Vec::new();
        let mut timestamps = Vec::new();

        // Steep uptrend
        for i in 0..50 {
            highs.push(100.0 + (i as f64) * 2.0);
            lows.push(90.0 + (i as f64) * 2.0);
            closes.push(95.0 + (i as f64) * 2.0);
            timestamps.push((i as i64) * 1000);
        }

        // Flat trend, reducing volatility => contracting
        for i in 50..60 {
            highs.push(200.0);
            lows.push(190.0);
            closes.push(195.0);
            timestamps.push((i as i64) * 1000);
        }

        let data = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes,
            "timestamp_unix_ms" => timestamps,
        ).unwrap();

        let config = GatorOscillatorConfig::default();
        let strategy = GatorOscillator::new(config);
        let signals = strategy.generate_signals(&data).await.unwrap();

        // Should detect contraction
        let entry_signals: Vec<_> = signals.iter().filter(|s| s.signal_type == SignalType::Entry).collect();
        assert!(!entry_signals.is_empty(), "Expected entry signal due to contraction");

        if let Some(signal) = entry_signals.last() {
            assert_eq!(signal.side, "sell");
            assert_eq!(signal.reason, "Gator histograms contracting");
        }
    }
}
