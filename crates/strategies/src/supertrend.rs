use crate::strategy::{Signal, SignalType, Strategy};
use crate::indicators::atr;
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupertrendConfig {
    pub period: usize,
    pub factor: f64,
    pub symbol: String,
}

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

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let period = self.config.period;
        let factor = self.config.factor;

        let atr_series = atr::calculate(data, period)?;
        let atr_values = atr_series.f64()?;

        let close_series = data.column("close")?.f64()?;
        let high_series = data.column("high")?.f64()?;
        let low_series = data.column("low")?.f64()?;
        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let len = data.height();

        if len < period {
            return Ok(signals);
        }

        // Find first valid index for ATR
        let mut first_valid_index = 0;
        while first_valid_index < len {
            if atr_values.get(first_valid_index).is_some() {
                break;
            }
            first_valid_index += 1;
        }

        if first_valid_index >= len {
            return Ok(signals);
        }

        // Initialize state at first_valid_index
        let idx = first_valid_index;
        let h = high_series.get(idx).unwrap_or(0.0);
        let l = low_series.get(idx).unwrap_or(0.0);
        let c = close_series.get(idx).unwrap_or(0.0);
        let atr_val = atr_values.get(idx).unwrap_or(0.0);

        let basic_upper = (h + l) / 2.0 + factor * atr_val;
        let basic_lower = (h + l) / 2.0 - factor * atr_val;

        let mut prev_final_upper_band = basic_upper;
        let mut prev_final_lower_band = basic_lower;

        let mut prev_trend = 1; // Default Up
        if c < basic_lower {
            prev_trend = -1;
        }

        // Loop from next index
        for i in (first_valid_index + 1)..len {
            let h = high_series.get(i).unwrap_or(0.0);
            let l = low_series.get(i).unwrap_or(0.0);
            let c = close_series.get(i).unwrap_or(0.0);
            let prev_c = close_series.get(i - 1).unwrap_or(0.0);

            let atr_val = match atr_values.get(i) {
                Some(v) => v,
                None => continue,
            };

            let timestamp = timestamps.get(i).unwrap_or(0);

            let basic_upper = (h + l) / 2.0 + factor * atr_val;
            let basic_lower = (h + l) / 2.0 - factor * atr_val;

            // Calculate Final Upper Band
            let final_upper = if basic_upper < prev_final_upper_band || prev_c > prev_final_upper_band {
                basic_upper
            } else {
                prev_final_upper_band
            };

            // Calculate Final Lower Band
            let final_lower = if basic_lower > prev_final_lower_band || prev_c < prev_final_lower_band {
                basic_lower
            } else {
                prev_final_lower_band
            };

            // Determine Trend
            let mut trend = prev_trend;
            if prev_trend == -1 && c > final_upper {
                trend = 1; // Trend Up
            } else if prev_trend == 1 && c < final_lower {
                trend = -1; // Trend Down
            }

            // Determine Supertrend Value
            let supertrend = if trend == 1 { final_lower } else { final_upper };

            // Generate Signals on Trend Change
            if trend != prev_trend {
                if trend == 1 {
                    // Trend changed to UP -> Buy Signal
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(supertrend),
                        take_profit: None,
                        reason: format!("Supertrend Flip Up (Price {:.2} > Upper Band {:.2})", c, final_upper),
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
                        reason: format!("Supertrend Flip Down (Price {:.2} < Lower Band {:.2})", c, final_lower),
                        timestamp_ms: timestamp,
                    });
                }
            }

            prev_final_upper_band = final_upper;
            prev_final_lower_band = final_lower;
            prev_trend = trend;
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
        let highs  = vec![101.0, 103.0, 105.0, 103.0, 99.0, 97.0, 101.0, 106.0];
        let lows   = vec![99.0,  101.0, 103.0, 101.0, 97.0, 95.0, 99.0,  104.0];
        let times: Vec<i64> = vec![1000,  2000,  3000,  4000,  5000, 6000, 7000,  8000];

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
