use crate::indicators::{atr, williams_fractal};
use crate::strategy::{Signal, SignalType, Strategy, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FractalBreakoutConfig {
    pub window: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for FractalBreakoutConfig {
    fn default() -> self::FractalBreakoutConfig {
        Self {
            window: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

pub struct FractalBreakout {
    config: FractalBreakoutConfig,
}

impl FractalBreakout {
    pub fn new(config: FractalBreakoutConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for FractalBreakout {
    fn name(&self) -> &str {
        "FractalBreakout"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let mut signals = Vec::new();
        if data.height() < self.config.window * 2 + 1 {
            return Ok(signals);
        }

        let close = data.column("close")?.f64()?;
        let high = data.column("high")?.f64()?;
        let low = data.column("low")?.f64()?;
        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        let (bullish_fractal_series, bearish_fractal_series) =
            williams_fractal::calculate(data, self.config.window)?;
        let bullish_fractal = bullish_fractal_series.bool()?;
        let bearish_fractal = bearish_fractal_series.bool()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr = atr_series.f64()?;

        let mut last_bull_low = f64::MIN; // Trailing support level
        let mut last_bear_high = f64::MAX; // Breakout resistance level
        let mut in_position = false;

        // Start from window to avoid out-of-bounds indexing.
        for i in self.config.window..data.height() {
            // A fractal formed at index `i - window` is strictly confirmed at index `i`
            let fractal_idx = i - self.config.window;

            // Check if a bearish fractal was confirmed
            if let Some(true) = bearish_fractal.get(fractal_idx) {
                if let Some(h) = high.get(fractal_idx) {
                    last_bear_high = h;
                }
            }

            // Check if a bullish fractal was confirmed
            if let Some(true) = bullish_fractal.get(fractal_idx) {
                // We use the low of the confirmed bullish fractal.
                if let Some(l) = low.get(fractal_idx) {
                    last_bull_low = l;
                }
            }

            let current_close = if let Some(c) = close.get(i) {
                c
            } else {
                continue;
            };

            let current_atr = atr.get(i).unwrap_or(0.0);
            let ts = timestamps.get(i).unwrap_or(0);

            // Entry logic: price breaks above recent bearish fractal
            if !in_position && current_close > last_bear_high && last_bear_high != f64::MAX {
                let stop_loss = current_close - (current_atr * self.config.stop_loss_atr_mult);
                signals.push(Signal {
                    signal_type: SignalType::Entry,
                    symbol: self.config.symbol.clone(),
                    side: "buy".to_string(),
                    size_hint: "100".to_string(),
                    confidence: 0.8,
                    stop_loss: Some(stop_loss),
                    take_profit: None,
                    reason: "Fractal Breakout (Bullish)".to_string(),
                    timestamp_ms: ts,
                });
                in_position = true;
            }

            // Exit logic: price breaks below recent bullish fractal
            if in_position && current_close < last_bull_low && last_bull_low != f64::MIN {
                signals.push(Signal {
                    signal_type: SignalType::Exit,
                    symbol: self.config.symbol.clone(),
                    side: "sell".to_string(),
                    size_hint: "max".to_string(),
                    confidence: 0.8,
                    stop_loss: None,
                    take_profit: None,
                    reason: "Fractal Breakdown (Bearish)".to_string(),
                    timestamp_ms: ts,
                });
                in_position = false;
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: FractalBreakoutConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn get_test_df() -> DataFrame {
        df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000],
            "open" => &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            "high" => &[1.0, 2.0, 3.0, 2.0, 1.0, 1.0, 1.0, 1.0], // Bearish fractal at index 2 (val = 3.0)
            "low" => &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            "close" => &[1.0, 1.0, 1.0, 1.0, 1.0, 4.0, 1.0, 1.0], // Close breaks above 3.0 at index 5
            "volume" => &[100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0]
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_fractal_breakout_entry() {
        let strategy = FractalBreakout::new(FractalBreakoutConfig {
            window: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let df = get_test_df();
        let signals = strategy.generate_signals(&df).await.unwrap();

        assert!(!signals.is_empty());
        assert_eq!(signals[0].signal_type, SignalType::Entry);
        assert_eq!(signals[0].symbol, "TEST");
        assert_eq!(signals[0].side, "buy");
    }

    #[tokio::test]
    async fn test_fractal_breakout_exit() {
        // Here we want to trigger an Entry and then an Exit
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000, 11000, 12000, 13000],
            "open" => &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            // Bearish fractal at index 2 (val = 3.0), confirmed at index 4
            "high" => &[1.0, 2.0, 3.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            // Bullish fractal at index 7 (val = 0.5), confirmed at index 9
            "low"  => &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.75, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0],
            // Entry at index 5 (close 4.0 > 3.0). Exit at index 11 (close 0.2 < 0.5)
            "close" => &[1.0, 1.0, 1.0, 1.0, 1.0, 4.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.2, 1.0],
            "volume" => &[100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0]
        )
        .unwrap();

        let strategy = FractalBreakout::new(FractalBreakoutConfig {
            window: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let signals = strategy.generate_signals(&df).await.unwrap();

        assert_eq!(signals.len(), 2);
        assert_eq!(signals[0].signal_type, SignalType::Entry);
        assert_eq!(signals[1].signal_type, SignalType::Exit);
        assert_eq!(signals[1].side, "sell");
        assert_eq!(signals[1].size_hint, "max");
    }

    #[tokio::test]
    async fn test_empty_dataframe() {
        let strategy = FractalBreakout::new(FractalBreakoutConfig::default());
        let df = df!(
            "timestamp_unix_ms" => &[] as &[i64],
            "high" => &[] as &[f64],
            "low" => &[] as &[f64],
            "close" => &[] as &[f64]
        )
        .unwrap();

        let signals = strategy.generate_signals(&df).await.unwrap();
        assert!(signals.is_empty());
    }

    #[tokio::test]
    async fn test_update_params() {
        let mut strategy = FractalBreakout::new(FractalBreakoutConfig::default());
        let valid_json = serde_json::json!({
            "window": 3,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "BTC".to_string()
        });

        assert!(strategy.update_params(valid_json).await.is_ok());
        assert_eq!(strategy.config.window, 3);
        assert_eq!(strategy.config.symbol, "BTC");

        let invalid_json = serde_json::json!({
            "window": "three"
        });
        assert!(strategy.update_params(invalid_json).await.is_err());
    }
}
