use crate::indicators::{atr, disparity_index};
use crate::strategy::{Signal, SignalType, Strategy, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde_json::Value;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DisparityIndexReversionConfig {
    pub period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub max_position_size: f64,
    pub symbol: String,
}

impl Default for DisparityIndexReversionConfig {
    fn default() -> Self {
        Self {
            period: 14,
            oversold_threshold: -5.0,
            overbought_threshold: 5.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            max_position_size: 100.0,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

pub struct DisparityIndexReversion {
    config: DisparityIndexReversionConfig,
}

impl DisparityIndexReversion {
    pub fn new(config: DisparityIndexReversionConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for DisparityIndexReversion {
    fn name(&self) -> &str {
        "DisparityIndexReversion"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let mut signals = Vec::new();

        if data.height() == 0 {
            return Ok(signals);
        }

        let closes = data.column("close")?.f64()?;
        // Try getting ms timestamp or fallback to regular timestamp
        let timestamps = if let Ok(col) = data.column("timestamp_ms") {
            col.i64()?.clone()
        } else if let Ok(col) = data.column("timestamp") {
            col.i64()?.clone()
        } else {
            // Generate dummy timestamps
            Int64Chunked::from_iter(std::iter::repeat_n(Some(0), data.height()))
        };

        // Calculate Indicators
        let di_values = disparity_index::calculate(data, self.config.period)?;
        let atr_values = atr::calculate(data, self.config.atr_period)?;

        let mut current_position = 0; // 0: None, 1: Long, -1: Short
        let mut current_sl = 0.0;
        let mut current_tp = 0.0;

        for i in 0..data.height() {
            let ts = timestamps.get(i).unwrap_or(0);

            let close_val = closes.get(i).unwrap_or(0.0);

            // Handle unwrapping AnyValue to f64 safely
            let get_f64 = |val: AnyValue| -> f64 {
                match val {
                    AnyValue::Float64(f) => f,
                    AnyValue::Float32(f) => f as f64,
                    AnyValue::Int64(i) => i as f64,
                    AnyValue::Int32(i) => i as f64,
                    _ => 0.0,
                }
            };

            let di_val = get_f64(di_values.get(i).unwrap_or(AnyValue::Null));
            let prev_di_val = if i > 0 {
                get_f64(di_values.get(i - 1).unwrap_or(AnyValue::Null))
            } else {
                0.0
            };
            let atr_val = get_f64(atr_values.get(i).unwrap_or(AnyValue::Null));
            let sl_dist = atr_val * self.config.stop_loss_atr_mult;

            match current_position {
                0 => {
                    // Long Entry: DI crosses below oversold threshold
                    if prev_di_val >= self.config.oversold_threshold
                        && di_val < self.config.oversold_threshold
                    {
                        let sl = close_val - sl_dist;
                        let tp = close_val + (sl_dist * 2.0);
                        let size = format!("{:.2}", self.config.max_position_size);
                        signals.push(Signal {
                            timestamp_ms: ts,
                            signal_type: SignalType::Entry,
                            side: "buy".to_string(),
                            symbol: self.config.symbol.clone(),
                            size_hint: size.clone(),
                            stop_loss: Some(sl),
                            take_profit: Some(tp),
                            confidence: 0.8,
                            reason: "DI Oversold".to_string(),
                        });
                        current_position = 1;
                        current_sl = sl;
                        current_tp = tp;
                    }
                    // Short Entry: DI crosses above overbought threshold
                    else if prev_di_val <= self.config.overbought_threshold
                        && di_val > self.config.overbought_threshold
                    {
                        let sl = close_val + sl_dist;
                        let tp = close_val - (sl_dist * 2.0);
                        let size = format!("{:.2}", self.config.max_position_size);
                        signals.push(Signal {
                            timestamp_ms: ts,
                            signal_type: SignalType::Entry,
                            side: "sell".to_string(),
                            symbol: self.config.symbol.clone(),
                            size_hint: size.clone(),
                            stop_loss: Some(sl),
                            take_profit: Some(tp),
                            confidence: 0.8,
                            reason: "DI Overbought".to_string(),
                        });
                        current_position = -1;
                        current_sl = sl;
                        current_tp = tp;
                    }
                }
                1 => {
                    // Long Exit: DI returns to mean (crosses above 0) or Stop Loss hit or Take Profit hit
                    if (prev_di_val < 0.0 && di_val >= 0.0)
                        || close_val <= current_sl
                        || close_val >= current_tp
                    {
                        signals.push(Signal {
                            timestamp_ms: ts,
                            signal_type: SignalType::Exit,
                            side: "sell".to_string(),
                            symbol: self.config.symbol.clone(),
                            size_hint: "max".to_string(),
                            stop_loss: None,
                            take_profit: None,
                            confidence: 1.0,
                            reason: "DI Mean Reversion or SL/TP".to_string(),
                        });
                        current_position = 0;
                    }
                }
                -1 => {
                    // Short Exit: DI returns to mean (crosses below 0) or Stop Loss hit or Take profit hit
                    if (prev_di_val > 0.0 && di_val <= 0.0)
                        || close_val >= current_sl
                        || close_val <= current_tp
                    {
                        signals.push(Signal {
                            timestamp_ms: ts,
                            signal_type: SignalType::Exit,
                            side: "buy".to_string(),
                            symbol: self.config.symbol.clone(),
                            size_hint: "max".to_string(),
                            stop_loss: None,
                            take_profit: None,
                            confidence: 1.0,
                            reason: "DI Mean Reversion or SL/TP".to_string(),
                        });
                        current_position = 0;
                    }
                }
                _ => {}
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: Value) -> Result<()> {
        let new_config: DisparityIndexReversionConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_dataframe() -> DataFrame {
        // We need enough data for the SMA(14) in DI to start generating values, and ATR(14).
        // Let's create a trend then a sudden drop to trigger an oversold condition.
        let mut closes = vec![100.0; 20];
        closes.extend(vec![95.0, 90.0, 85.0, 80.0, 75.0]); // Sudden drop to cause high negative DI
        closes.extend(vec![85.0, 90.0, 95.0, 100.0, 105.0]); // Reversion to mean

        // Also need a peak to trigger short entry
        closes.extend(vec![110.0, 120.0, 130.0, 140.0]); // Sudden rise
        closes.extend(vec![130.0, 120.0, 110.0, 100.0]); // Reversion

        let n = closes.len();
        let highs: Vec<f64> = closes.iter().map(|&c| c + 2.0).collect();
        let lows: Vec<f64> = closes.iter().map(|&c| c - 2.0).collect();
        let timestamps: Vec<i64> = (0..n).map(|i| (i as i64) * 86400000).collect();

        df!(
            "timestamp" => timestamps,
            "open" => closes.clone(),
            "high" => highs,
            "low" => lows,
            "close" => closes,
            "volume" => vec![1000.0; n],
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_entry_signal_generation() {
        let df = create_test_dataframe();
        let config = DisparityIndexReversionConfig {
            period: 14,
            oversold_threshold: -5.0, // Look for 5% drop below SMA
            overbought_threshold: 5.0,
            symbol: "TEST".to_string(),
            ..Default::default()
        };
        let strategy = DisparityIndexReversion::new(config);

        let signals = strategy.generate_signals(&df).await.unwrap();

        assert!(!signals.is_empty(), "Should generate signals");

        // Find the first long signal (Entry Buy)
        let long_signal = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "buy");
        assert!(long_signal.is_some(), "Should generate a long signal");

        if let Some(signal) = long_signal {
            assert_eq!(signal.symbol, "TEST");
            assert!(signal.stop_loss.is_some());
            assert!(signal.take_profit.is_some());
        }

        // Find a short signal (Entry Sell)
        let short_signal = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "sell");
        assert!(short_signal.is_some(), "Should generate a short signal");
    }

    #[tokio::test]
    async fn test_exit_signal_generation() {
        let df = create_test_dataframe();
        let config = DisparityIndexReversionConfig {
            period: 14,
            oversold_threshold: -5.0,
            overbought_threshold: 5.0,
            symbol: "TEST".to_string(),
            ..Default::default()
        };
        let strategy = DisparityIndexReversion::new(config);

        let signals = strategy.generate_signals(&df).await.unwrap();

        // There should be an Exit Sell (closing the long)
        let has_close_long = signals
            .iter()
            .any(|s| s.signal_type == SignalType::Exit && s.side == "sell");
        assert!(
            has_close_long,
            "Should generate a close long signal as price reverts"
        );

        // And an Exit Buy (closing the short)
        let has_close_short = signals
            .iter()
            .any(|s| s.signal_type == SignalType::Exit && s.side == "buy");
        assert!(has_close_short, "Should generate a close short signal");
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let config = DisparityIndexReversionConfig::default();
        let strategy = DisparityIndexReversion::new(config.clone());
        assert_eq!(strategy.name(), "DisparityIndexReversion");
    }

    #[tokio::test]
    async fn test_edge_cases() {
        let config = DisparityIndexReversionConfig::default();
        let strategy = DisparityIndexReversion::new(config);

        // Empty dataframe
        let empty_df = DataFrame::empty();
        let signals = strategy.generate_signals(&empty_df).await.unwrap();
        assert!(signals.is_empty());
    }
}
