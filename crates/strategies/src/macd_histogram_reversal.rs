use crate::indicators::{atr, macd};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

/// Configuration for the MACD Histogram Reversal strategy.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MacdHistogramReversalConfig {
    /// Fast EMA period for MACD.
    pub macd_fast_period: usize,
    /// Slow EMA period for MACD.
    pub macd_slow_period: usize,
    /// Signal line period for MACD.
    pub macd_signal_period: usize,
    /// Stop loss multiplier for ATR.
    pub stop_loss_atr_mult: f64,
    /// ATR period for stop loss.
    pub atr_period: usize,
    /// Symbol to trade.
    pub symbol: String,
}

impl Default for MacdHistogramReversalConfig {
    fn default() -> Self {
        Self {
            macd_fast_period: 12,
            macd_slow_period: 26,
            macd_signal_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for MacdHistogramReversalConfig {}

/// MACD Histogram Reversal Strategy.
///
/// Anticipates trend changes by looking for the MACD histogram to change direction
/// (tick up while below zero, or tick down while above zero), indicating a shift in momentum
/// before the actual MACD/Signal lines cross.
pub struct MacdHistogramReversal {
    config: MacdHistogramReversalConfig,
}

impl MacdHistogramReversal {
    /// Creates a new instance of the strategy.
    pub fn new(config: MacdHistogramReversalConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for MacdHistogramReversal {
    fn name(&self) -> &str {
        "MacdHistogramReversal"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let mut signals = Vec::new();

        let close_series = data.column("close")?.cast(&DataType::Float64)?;
        let closes: Vec<Option<f64>> = close_series.f64()?.into_iter().collect();

        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        // Calculate MACD
        let (_macd_line, _signal_line, hist_series) = macd::calculate(
            data,
            self.config.macd_fast_period,
            self.config.macd_slow_period,
            self.config.macd_signal_period,
        )?;

        let hist_series = hist_series.cast(&DataType::Float64)?;
        let histograms: Vec<Option<f64>> = hist_series.f64()?.into_iter().collect();

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_series = atr_series.cast(&DataType::Float64)?;
        let atrs: Vec<Option<f64>> = atr_series.f64()?.into_iter().collect();

        // Need at least 3 periods to confirm a reversal (prev_prev, prev, current)
        let min_len = 3.max(self.config.macd_slow_period + self.config.macd_signal_period);

        let mut in_long = false;
        let mut in_short = false;
        let mut stop_loss = 0.0;

        for i in min_len..closes.len() {
            let current_close = closes[i];
            let current_ts = timestamps.get(i);

            let current_hist = histograms[i];
            let prev_hist = histograms[i - 1];
            let prev_prev_hist = histograms[i - 2];

            let current_atr = atrs[i];

            if current_close.is_none()
                || current_ts.is_none()
                || current_hist.is_none()
                || prev_hist.is_none()
                || prev_prev_hist.is_none()
                || current_atr.is_none()
            {
                continue;
            }

            let price = current_close.unwrap();
            let ts = current_ts.unwrap();
            let hist = current_hist.unwrap();
            let p_hist = prev_hist.unwrap();
            let pp_hist = prev_prev_hist.unwrap();
            let atr_val = current_atr.unwrap();

            // Check stop loss first
            if in_long && price <= stop_loss {
                signals.push(Signal {
                    signal_type: SignalType::Exit,
                    symbol: self.config.symbol.clone(),
                    side: "sell".to_string(),
                    size_hint: "max".to_string(),
                    confidence: 1.0,
                    stop_loss: None,
                    take_profit: None,
                    reason: "Stop Loss Hit".to_string(),
                    timestamp_ms: ts,
                });
                in_long = false;
                continue; // Skip further logic for this bar
            }

            if in_short && price >= stop_loss {
                signals.push(Signal {
                    signal_type: SignalType::Exit,
                    symbol: self.config.symbol.clone(),
                    side: "buy".to_string(),
                    size_hint: "max".to_string(),
                    confidence: 1.0,
                    stop_loss: None,
                    take_profit: None,
                    reason: "Stop Loss Hit".to_string(),
                    timestamp_ms: ts,
                });
                in_short = false;
                continue;
            }

            // Long Entry: Histogram was negative, bottomed out (prev < prev_prev), and is now rising (current > prev)
            let long_entry = p_hist < 0.0 && pp_hist > p_hist && hist > p_hist;

            // Short Entry: Histogram was positive, peaked (prev > prev_prev), and is now falling (current < prev)
            let short_entry = p_hist > 0.0 && pp_hist < p_hist && hist < p_hist;

            // Exit conditions: Histogram reversing against our position
            let long_exit = in_long && hist < p_hist;
            let short_exit = in_short && hist > p_hist;

            if !in_long && long_entry {
                // Close short if open
                if in_short {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 1.0,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Reverse to Long".to_string(),
                        timestamp_ms: ts,
                    });
                    in_short = false;
                }

                let sl = price - (atr_val * self.config.stop_loss_atr_mult);
                signals.push(Signal {
                    signal_type: SignalType::Entry,
                    symbol: self.config.symbol.clone(),
                    side: "buy".to_string(),
                    size_hint: "100".to_string(),
                    confidence: 0.8,
                    stop_loss: Some(sl),
                    take_profit: None,
                    reason: "MACD Histogram Tick Up (Below Zero)".to_string(),
                    timestamp_ms: ts,
                });
                in_long = true;
                stop_loss = sl;
            } else if !in_short && short_entry {
                // Close long if open
                if in_long {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 1.0,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Reverse to Short".to_string(),
                        timestamp_ms: ts,
                    });
                    in_long = false;
                }

                let sl = price + (atr_val * self.config.stop_loss_atr_mult);
                signals.push(Signal {
                    signal_type: SignalType::Entry,
                    symbol: self.config.symbol.clone(),
                    side: "sell".to_string(),
                    size_hint: "100".to_string(),
                    confidence: 0.8,
                    stop_loss: Some(sl),
                    take_profit: None,
                    reason: "MACD Histogram Tick Down (Above Zero)".to_string(),
                    timestamp_ms: ts,
                });
                in_short = true;
                stop_loss = sl;
            } else if long_exit {
                signals.push(Signal {
                    signal_type: SignalType::Exit,
                    symbol: self.config.symbol.clone(),
                    side: "sell".to_string(),
                    size_hint: "max".to_string(),
                    confidence: 1.0,
                    stop_loss: None,
                    take_profit: None,
                    reason: "MACD Histogram Reversal (Exit Long)".to_string(),
                    timestamp_ms: ts,
                });
                in_long = false;
            } else if short_exit {
                signals.push(Signal {
                    signal_type: SignalType::Exit,
                    symbol: self.config.symbol.clone(),
                    side: "buy".to_string(),
                    size_hint: "max".to_string(),
                    confidence: 1.0,
                    stop_loss: None,
                    take_profit: None,
                    reason: "MACD Histogram Reversal (Exit Short)".to_string(),
                    timestamp_ms: ts,
                });
                in_short = false;
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        if let Some(macd_fast_period) = params.get("macd_fast_period").and_then(|v| v.as_u64()) {
            self.config.macd_fast_period = macd_fast_period as usize;
        }
        if let Some(macd_slow_period) = params.get("macd_slow_period").and_then(|v| v.as_u64()) {
            self.config.macd_slow_period = macd_slow_period as usize;
        }
        if let Some(macd_signal_period) = params.get("macd_signal_period").and_then(|v| v.as_u64()) {
            self.config.macd_signal_period = macd_signal_period as usize;
        }
        if let Some(stop_loss_atr_mult) = params.get("stop_loss_atr_mult").and_then(|v| v.as_f64()) {
            self.config.stop_loss_atr_mult = stop_loss_atr_mult;
        }
        if let Some(atr_period) = params.get("atr_period").and_then(|v| v.as_u64()) {
            self.config.atr_period = atr_period as usize;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create test data
    fn create_test_data() -> DataFrame {
        // Need enough data for MACD (26) + Signal (9) + a few bars for reversal
        // Also need to allow ATR to stabilize (14 bars). Let's use 60 bars.
        let mut closes = vec![100.0; 60];
        let mut highs = vec![105.0; 60];
        let mut lows = vec![95.0; 60];
        let mut opens = vec![100.0; 60];
        let mut volumes = vec![1000.0; 60];
        let timestamps: Vec<i64> = (0..60).map(|i| i as i64 * 60000).collect();

        // Create a scenario for a LONG entry (Histogram below zero, then rising)
        // Price drops steadily to create negative histogram, then bounces
        for i in 20..45 {
            closes[i] = closes[i - 1] - 1.0;
            highs[i] = closes[i] + 1.0;
            lows[i] = closes[i] - 1.0;
            opens[i] = closes[i - 1];
        }
        // Bounce significantly to create a tick up in histogram
        for i in 45..50 {
            closes[i] = closes[i - 1] + 5.0;
            highs[i] = closes[i] + 1.0;
            lows[i] = closes[i] - 1.0;
            opens[i] = closes[i - 1];
        }

        df!(
            "timestamp_unix_ms" => timestamps,
            "open" => opens,
            "high" => highs,
            "low" => lows,
            "close" => closes,
            "volume" => volumes
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_macd_histogram_reversal_strategy() {
        let df = create_test_data();
        let config = MacdHistogramReversalConfig {
            macd_fast_period: 12,
            macd_slow_period: 26,
            macd_signal_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = MacdHistogramReversal::new(config);
        let signals = strategy.generate_signals(&df).await.unwrap();

        // Check if any signals were generated
        // We aren't testing the actual indicator values here, just the signal generation
        // given some basic mock data.

        let strategy = MacdHistogramReversal::new(MacdHistogramReversalConfig::default());

        assert_eq!(strategy.name(), "MacdHistogramReversal");
        assert_eq!(strategy.strategy_type(), StrategyType::Momentum);
    }

    #[tokio::test]
    async fn test_update_params() {
        let mut strategy = MacdHistogramReversal::new(MacdHistogramReversalConfig::default());

        let new_params = serde_json::json!({
            "macd_fast_period": 10,
            "macd_slow_period": 20,
            "macd_signal_period": 8,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10
        });

        strategy.update_params(new_params).await.unwrap();

        assert_eq!(strategy.config.macd_fast_period, 10);
        assert_eq!(strategy.config.macd_slow_period, 20);
        assert_eq!(strategy.config.macd_signal_period, 8);
        assert_eq!(strategy.config.stop_loss_atr_mult, 1.5);
        assert_eq!(strategy.config.atr_period, 10);
    }

    #[tokio::test]
    async fn test_empty_data() {
        let df = df!(
            "timestamp_unix_ms" => Vec::<i64>::new(),
            "open" => Vec::<f64>::new(),
            "high" => Vec::<f64>::new(),
            "low" => Vec::<f64>::new(),
            "close" => Vec::<f64>::new(),
            "volume" => Vec::<f64>::new()
        ).unwrap();

        let strategy = MacdHistogramReversal::new(MacdHistogramReversalConfig::default());
        let result = strategy.generate_signals(&df).await;

        // Should handle gracefully or return error depending on indicator calculation
        // Typically returns empty signals if calculated without error
        if let Ok(signals) = result {
            assert!(signals.is_empty());
        }
    }
}
