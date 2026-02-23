use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BollingerBandsConfig {
    pub window_size: usize,
    pub num_std_dev: f64,
    pub stop_loss_pct: f64,
    pub symbol: String,
}

impl StrategyConfig for BollingerBandsConfig {}

pub struct BollingerBandsMeanReversion {
    config: BollingerBandsConfig,
}

impl BollingerBandsMeanReversion {
    pub fn new(config: BollingerBandsConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for BollingerBandsMeanReversion {
    fn name(&self) -> &str {
        "BollingerBandsMeanReversion"
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let window_size = self.config.window_size;
        let mut signals = Vec::new();

        if close_arr.len() < window_size {
            return Ok(signals);
        }

        let mut current_sum = 0.0;
        let mut current_sum_sq = 0.0;

        // Initialize first window (0 to window_size - 1)
        for i in 0..window_size {
            if let Some(val) = close_arr.get(i) {
                current_sum += val;
                current_sum_sq += val * val;
            }
        }

        // We track prev_mean for crossover detection.
        // Initialize prev_mean from the first window check.
        let mut prev_mean: f64;

        // Check first window
        {
            let i = window_size - 1;
            let mean = current_sum / window_size as f64;
            let variance = (current_sum_sq / window_size as f64) - (mean * mean);
            let std_dev = if variance < 0.0 { 0.0 } else { variance.sqrt() };
            let upper = mean + (std_dev * self.config.num_std_dev);
            let lower = mean - (std_dev * self.config.num_std_dev);

            prev_mean = mean;

            if let (Some(close), Some(ts)) = (close_arr.get(i), time_arr.get(i)) {
                // Initial window signal check
                 if close < lower {
                    // Check for ScaleIn depth
                     let signal_type = if close < lower - (0.5 * std_dev) {
                         SignalType::ScaleIn
                     } else {
                         SignalType::Entry
                     };
                    signals.push(Signal {
                        signal_type,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        reason: format!("Close {:.2} < Lower Band {:.2}", close, lower),
                        timestamp_ms: ts,
                    });
                } else if close > upper {
                     let signal_type = if close > upper + (0.5 * std_dev) {
                         SignalType::ScaleIn
                     } else {
                         SignalType::Entry
                     };
                    signals.push(Signal {
                        signal_type,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Short Entry
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        reason: format!("Close {:.2} > Upper Band {:.2}", close, upper),
                        timestamp_ms: ts,
                    });
                }
            }
        }

        // Slide window
        for i in window_size..close_arr.len() {
            if let Some(old_val) = close_arr.get(i - window_size) {
                current_sum -= old_val;
                current_sum_sq -= old_val * old_val;
            }

            if let Some(new_val) = close_arr.get(i) {
                current_sum += new_val;
                current_sum_sq += new_val * new_val;

                let mean = current_sum / window_size as f64;
                let variance = (current_sum_sq / window_size as f64) - (mean * mean);
                let std_dev = if variance < 0.0 { 0.0 } else { variance.sqrt() };

                let upper = mean + (std_dev * self.config.num_std_dev);
                let lower = mean - (std_dev * self.config.num_std_dev);

                let prev_close = close_arr.get(i-1).unwrap(); // Safe because i starts at window_size >= 1

                if let Some(ts) = time_arr.get(i) {
                    // Entry / ScaleIn Logic
                    if new_val < lower {
                         let signal_type = if new_val < lower - (0.5 * std_dev) {
                             SignalType::ScaleIn
                         } else {
                             SignalType::Entry
                         };
                        signals.push(Signal {
                            signal_type,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            reason: format!("Close {:.2} < Lower Band {:.2}", new_val, lower),
                            timestamp_ms: ts,
                        });
                    } else if new_val > upper {
                         let signal_type = if new_val > upper + (0.5 * std_dev) {
                             SignalType::ScaleIn
                         } else {
                             SignalType::Entry
                         };
                        signals.push(Signal {
                            signal_type,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            reason: format!("Close {:.2} > Upper Band {:.2}", new_val, upper),
                            timestamp_ms: ts,
                        });
                    }

                    // Exit Logic (Mean Crossover)
                    // Exit Long (Sell): Price crosses SMA from below
                    // if prev_close < prev_mean && new_val >= mean
                    if prev_close < prev_mean && new_val >= mean {
                         signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(), // Exit Long
                            size_hint: "max".to_string(),
                            confidence: 0.6,
                            reason: format!("Price crossed SMA from below ({:.2} -> {:.2}, Mean {:.2})", prev_close, new_val, mean),
                            timestamp_ms: ts,
                        });
                    }

                    // Exit Short (Buy): Price crosses SMA from above
                    // if prev_close > prev_mean && new_val <= mean
                    if prev_close > prev_mean && new_val <= mean {
                         signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(), // Exit Short
                            size_hint: "max".to_string(),
                            confidence: 0.6,
                            reason: format!("Price crossed SMA from above ({:.2} -> {:.2}, Mean {:.2})", prev_close, new_val, mean),
                            timestamp_ms: ts,
                        });
                    }
                }

                prev_mean = mean;
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: BollingerBandsConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_generate_signals() -> Result<()> {
        let config = BollingerBandsConfig {
            window_size: 3,
            num_std_dev: 1.0, // Small std dev to trigger signals easily
            stop_loss_pct: 0.05,
            symbol: "AAPL".to_string(),
        };
        let strategy = BollingerBandsMeanReversion::new(config);

        // Pattern: 10, 10, 10 (mean 10, std 0) -> no signal
        // then 15 (mean ~11, std increase) -> 15 > upper -> Entry Sell Signal (Short)
        let df = df! (
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "close" => &[10.0, 10.0, 10.0, 15.0],
        )?;

        let signals = strategy.generate_signals(&df).await?;

        assert!(!signals.is_empty(), "Should generate signals");
        let last_signal = signals.last().unwrap();
        // Updated expectation: Entry (Short) instead of Exit
        assert_eq!(last_signal.signal_type, SignalType::Entry);
        assert_eq!(last_signal.side, "sell");

        Ok(())
    }

    #[tokio::test]
    async fn test_crossover_exit() -> Result<()> {
        let config = BollingerBandsConfig {
            window_size: 3,
            num_std_dev: 2.0,
            stop_loss_pct: 0.05,
            symbol: "AAPL".to_string(),
        };
        let strategy = BollingerBandsMeanReversion::new(config);

        // 10, 10, 10 -> Mean 10.
        // 8 -> Mean (10+10+8)/3 = 9.33. Close 8. (Below Mean)
        // 12 -> Mean (10+8+12)/3 = 10. Close 12. (Above Mean). Crossover!

        let df = df! (
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "close" => &[10.0, 10.0, 10.0, 8.0, 12.0],
        )?;

        let signals = strategy.generate_signals(&df).await?;
        // We expect an Exit signal at timestamp 5000.

        let exit_signal = signals.iter().find(|s| s.signal_type == SignalType::Exit);
        assert!(exit_signal.is_some());
        let s = exit_signal.unwrap();
        assert_eq!(s.timestamp_ms, 5000);
        assert_eq!(s.side, "sell"); // Crossed from below -> Exit Long -> Sell

        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let config = BollingerBandsConfig {
            window_size: 10,
            num_std_dev: 2.0,
            stop_loss_pct: 0.05,
            symbol: "AAPL".to_string(),
        };
        let mut strategy = BollingerBandsMeanReversion::new(config);

        let new_params = serde_json::json!({
            "window_size": 20,
            "num_std_dev": 2.5,
            "stop_loss_pct": 0.10,
            "symbol": "GOOG"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.window_size, 20);
        assert_eq!(strategy.config.symbol, "GOOG");
        Ok(())
    }
}
