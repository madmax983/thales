//! The Supertrend Strategy
//!
//! The Supertrend indicator is a trend-following indicator based on the Average True Range (ATR).
//! It is plotted on the price chart and indicates the current trend direction.
//!
//! - **Entry Signal:** A buy signal is generated when the price crosses above the Supertrend line, turning the indicator bullish.
//! - **Exit Signal:** A sell signal is generated when the price crosses below the Supertrend line, turning the indicator bearish.
//!
use crate::indicators::supertrend;
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration parameters for the `SupertrendStrategy` strategy.
///
/// # Examples
///
/// ```
/// use strategies::supertrend::SupertrendConfig;
///
/// let config = SupertrendConfig {
///     period: 14,
///     factor: 3.0,
///     symbol: "BTCUSD".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupertrendConfig {
    pub period: usize,
    pub factor: f64,
    pub symbol: String,
}

impl StrategyConfig for SupertrendConfig {}

pub struct Supertrend {
    config: SupertrendConfig,
}

impl Supertrend {
    pub fn new(config: SupertrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for Supertrend {
    fn name(&self) -> &str {
        "Supertrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let period = self.config.period;
        let factor = self.config.factor;

        // Use the new indicator implementation
        let (st_series, trend_series) = supertrend::calculate(data, period, factor)?;

        let st_values = st_series.f64()?;
        let trend_values = trend_series.i32()?;
        let close_series = data.column("close")?.f64()?;
        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let len = data.height();

        let mut prev_trend: Option<i32> = None;

        for i in 0..len {
            // Skip if trend is null (initial period)
            let trend = match trend_values.get(i) {
                Some(t) => t,
                None => continue,
            };

            if let Some(pt) = prev_trend {
                if trend != pt {
                    let timestamp = timestamps.get(i).unwrap_or(0);
                    let close = close_series.get(i).unwrap_or(0.0);
                    let st_val = st_values.get(i).unwrap_or(0.0);

                    if trend == 1 {
                        // Trend changed to UP -> Buy Signal
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(st_val),
                            take_profit: None,
                            reason: format!("Supertrend Flip Up (Price {:.2} > Upper Band)", close), // Simplified reason as we don't have previous band easily accessible here without extra lookups
                            timestamp_ms: timestamp,
                        });
                    } else if trend == -1 {
                        // Trend changed to DOWN -> Sell Signal
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            stop_loss: None,
                            take_profit: None,
                            reason: format!(
                                "Supertrend Flip Down (Price {:.2} < Lower Band)",
                                close
                            ),
                            timestamp_ms: timestamp,
                        });
                    }
                }
            }
            prev_trend = Some(trend);
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let config: SupertrendConfig = serde_json::from_value(params)?;
        self.config = config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_supertrend_calculation() -> Result<()> {
        // Create a simple dataset where trend flips
        // Period 2, Factor 1.0 (for simplicity)

        let closes = vec![100.0, 102.0, 104.0, 102.0, 98.0, 96.0, 100.0, 105.0];
        let highs = vec![101.0, 103.0, 105.0, 103.0, 99.0, 97.0, 101.0, 106.0];
        let lows = vec![99.0, 101.0, 103.0, 101.0, 97.0, 95.0, 99.0, 104.0];
        let times: Vec<i64> = vec![1000, 2000, 3000, 4000, 5000, 6000, 7000, 8000];

        let df = df!(
            "close" => closes,
            "high" => highs,
            "low" => lows,
            "timestamp_unix_ms" => times
        )?;

        let config = SupertrendConfig {
            period: 2,
            factor: 1.0,
            symbol: "TEST".to_string(),
        };
        let strategy = Supertrend::new(config);

        let signals = strategy.generate_signals(&df).await?;

        // Print signals for debugging
        for s in &signals {
            println!("{:?}", s);
        }

        // We expect an Exit (Flip Down) and an Entry (Flip Up)
        let has_exit = signals.iter().any(|s| s.signal_type == SignalType::Exit);
        assert!(has_exit, "Should have generated an Exit signal");

        let has_entry = signals.iter().any(|s| s.signal_type == SignalType::Entry);
        assert!(has_entry, "Should have generated an Entry signal");

        Ok(())
    }
}
