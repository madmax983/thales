use crate::indicators::{atr, linear_regression};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::{Context, Result};
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
enum PositionType {
    Long { stop_loss: f64 },
    Short { stop_loss: f64 },
}

pub struct LinearRegressionTrend {
    config: LinearRegressionTrendConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearRegressionTrendConfig {
    pub period: usize,
    pub slope_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl LinearRegressionTrend {
    pub fn new(config: LinearRegressionTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for LinearRegressionTrend {
    fn name(&self) -> &str {
        "LinearRegressionTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close = data.column("close").context("Missing close")?.f64()?;
        let timestamps = data
            .column("timestamp_unix_ms")
            .context("Missing timestamp_unix_ms")?
            .cast(&DataType::Int64)?
            .i64()?
            .clone();

        // 1. Log Prices for scale-invariant slope
        // We use unwrap_or(0.0) for safety but ideally we should handle NaN/Inf
        let log_close = close.apply(|v_opt| match v_opt {
            Some(v) if v > 0.0 => Some(v.ln()),
            _ => None,
        });
        let log_df = df!("close" => log_close.clone())?;

        // 2. Slope
        let slope_series = linear_regression::calculate(&log_df, self.config.period)
            .context("Failed to calculate Linear Regression Slope")?;
        let slope = slope_series.f64()?;

        // 3. ATR
        let atr_series =
            atr::calculate(data, self.config.atr_period).context("Failed to calculate ATR")?;
        let atr = atr_series.f64()?;

        let mut signals = Vec::new();
        let mut position: Option<PositionType> = None;

        // Iterate through data
        for i in self.config.period..close.len() {
            let ts = timestamps.get(i).unwrap_or(0);
            let s = slope.get(i);
            let a = atr.get(i);
            let p = close.get(i);

            if let (Some(slope_val), Some(atr_val), Some(price)) = (s, a, p) {
                let mut exit_signal = None;

                // Check Exits
                if let Some(pos) = &position {
                    match pos {
                        PositionType::Long { stop_loss } => {
                            if price <= *stop_loss {
                                exit_signal = Some(("Stop Loss", "sell"));
                            } else if slope_val < 0.0 {
                                exit_signal = Some(("Trend Reversal (Slope < 0)", "sell"));
                            }
                        }
                        PositionType::Short { stop_loss } => {
                            if price >= *stop_loss {
                                exit_signal = Some(("Stop Loss", "buy"));
                            } else if slope_val > 0.0 {
                                exit_signal = Some(("Trend Reversal (Slope > 0)", "buy"));
                            }
                        }
                    }
                }

                if let Some((reason, side)) = exit_signal {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: side.to_string(),
                        size_hint: "max".to_string(),
                        confidence: 1.0,
                        stop_loss: None,
                        take_profit: None,
                        reason: reason.to_string(),
                        timestamp_ms: ts,
                    });
                    position = None;
                }

                // Check Entries (only if no position)
                if position.is_none() {
                    if slope_val > self.config.slope_threshold {
                        // Long Entry
                        let sl = price - (atr_val * self.config.stop_loss_atr_mult);
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl),
                            take_profit: None,
                            reason: format!(
                                "Slope {:.5} > Threshold {}",
                                slope_val, self.config.slope_threshold
                            ),
                            timestamp_ms: ts,
                        });
                        position = Some(PositionType::Long { stop_loss: sl });
                    } else if slope_val < -self.config.slope_threshold {
                        // Short Entry
                        let sl = price + (atr_val * self.config.stop_loss_atr_mult);
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl),
                            take_profit: None,
                            reason: format!(
                                "Slope {:.5} < -Threshold {}",
                                slope_val, self.config.slope_threshold
                            ),
                            timestamp_ms: ts,
                        });
                        position = Some(PositionType::Short { stop_loss: sl });
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: LinearRegressionTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_entry_signal_generation() -> Result<()> {
        // Create an exponential trend: y = e^(0.1 * x)
        // log(y) = 0.1 * x. Slope of log(y) should be 0.1.
        // If threshold is 0.05, we should get Long signals.

        let mut values: Vec<f64> = Vec::new();
        let mut timestamps: Vec<i64> = Vec::new();
        for i in 0..50 {
            values.push((0.1 * i as f64).exp());
            timestamps.push(i as i64 * 1000);
        }

        // Needs High/Low/Close for ATR
        let highs: Vec<f64> = values.iter().map(|v| v + 1.0).collect();
        let lows: Vec<f64> = values.iter().map(|v| v - 1.0).collect();

        let df = df!(
            "close" => values,
            "high" => highs,
            "low" => lows,
            "timestamp_unix_ms" => timestamps
        )?;

        let config = LinearRegressionTrendConfig {
            period: 10,
            slope_threshold: 0.05,
            stop_loss_atr_mult: 2.0,
            atr_period: 5,
            symbol: "TEST".to_string(),
        };

        let strategy = LinearRegressionTrend::new(config);
        let signals = strategy.generate_signals(&df).await?;

        // Should have signals
        assert!(!signals.is_empty());

        // First signals should be Entry Long (buy)
        let first_signal = signals.first().unwrap();
        assert_eq!(first_signal.signal_type, SignalType::Entry);
        assert_eq!(first_signal.side, "buy");

        Ok(())
    }

    #[tokio::test]
    async fn test_exit_logic() -> Result<()> {
        // Up then Down
        // 0..30: Up, 30..60: Down
        let mut values: Vec<f64> = Vec::new();
        let mut timestamps: Vec<i64> = Vec::new();

        for i in 0..30 {
            values.push((0.1 * i as f64).exp());
            timestamps.push(i as i64 * 1000);
        }
        let peak = values.last().unwrap().clone();
        for i in 1..30 {
            values.push(peak * (-0.1 * i as f64).exp());
            timestamps.push((30 + i) as i64 * 1000);
        }

        let highs: Vec<f64> = values.iter().map(|v| v + 1.0).collect();
        let lows: Vec<f64> = values.iter().map(|v| v - 1.0).collect();

        let df = df!(
            "close" => values,
            "high" => highs,
            "low" => lows,
            "timestamp_unix_ms" => timestamps
        )?;

        let config = LinearRegressionTrendConfig {
            period: 5,
            slope_threshold: 0.01,
            stop_loss_atr_mult: 2.0,
            atr_period: 5,
            symbol: "TEST".to_string(),
        };

        let strategy = LinearRegressionTrend::new(config);
        let signals = strategy.generate_signals(&df).await?;

        // Should have Long Entry, then Long Exit (sell), then Short Entry (sell)
        let has_entry_long = signals
            .iter()
            .any(|s| s.signal_type == SignalType::Entry && s.side == "buy");
        let has_exit_long = signals
            .iter()
            .any(|s| s.signal_type == SignalType::Exit && s.side == "sell");
        let has_entry_short = signals
            .iter()
            .any(|s| s.signal_type == SignalType::Entry && s.side == "sell");

        assert!(has_entry_long, "Missing Long Entry");
        assert!(has_exit_long, "Missing Long Exit");
        assert!(has_entry_short, "Missing Short Entry");

        Ok(())
    }
}
