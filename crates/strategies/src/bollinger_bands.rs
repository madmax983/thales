use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
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

        let mut current_sum = Decimal::ZERO;
        let mut current_sum_sq = Decimal::ZERO;
        let window_size_dec = Decimal::from_usize(window_size).unwrap_or(Decimal::ONE);
        let num_std_dev_dec =
            Decimal::from_f64_retain(self.config.num_std_dev).unwrap_or(Decimal::ZERO);
        let half_dec = Decimal::new(5, 1); // 0.5
        let two_dec = Decimal::new(2, 0); // 2.0

        // Initialize first window (0 to window_size - 1)
        for i in 0..window_size {
            if let Some(val) = close_arr.get(i) {
                if let Some(d) = Decimal::from_f64_retain(val) {
                    current_sum += d;
                    current_sum_sq += d * d;
                }
            }
        }

        // We track prev_mean for crossover detection.
        // Initialize prev_mean from the first window check.
        let mut prev_mean: Decimal;

        // Check first window
        {
            let i = window_size - 1;
            let mean = current_sum / window_size_dec;
            let variance = (current_sum_sq / window_size_dec) - (mean * mean);
            // variance might be slightly negative due to precision if using f64, but with Decimal should be fine.
            // Safety check for sqrt.
            let std_dev = if variance <= Decimal::ZERO {
                Decimal::ZERO
            } else {
                variance.sqrt().unwrap_or(Decimal::ZERO)
            };

            let upper = mean + (std_dev * num_std_dev_dec);
            let lower = mean - (std_dev * num_std_dev_dec);

            prev_mean = mean;

            if let (Some(close_val), Some(ts)) = (close_arr.get(i), time_arr.get(i)) {
                if let Some(close) = Decimal::from_f64_retain(close_val) {
                    // Initial window signal check
                    if close < lower {
                        // Check for ScaleIn depth
                        let signal_type = if close < lower - (half_dec * std_dev) {
                            SignalType::ScaleIn
                        } else {
                            SignalType::Entry
                        };

                        // Calculate SL/TP
                        // Buy: SL below close (e.g. 2 std dev further down?) or just use config SL pct
                        // Let's use 2 std dev below close for now as per plan
                        let sl = close - (two_dec * std_dev);
                        let tp = mean;

                        signals.push(Signal {
                            signal_type,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                            reason: format!("Close {} < Lower Band {}", close, lower.round_dp(2)),
                            timestamp_ms: ts,
                        });
                    } else if close > upper {
                        let signal_type = if close > upper + (half_dec * std_dev) {
                            SignalType::ScaleIn
                        } else {
                            SignalType::Entry
                        };

                        let sl = close + (two_dec * std_dev);
                        let tp = mean;

                        signals.push(Signal {
                            signal_type,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(), // Short Entry
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                            reason: format!("Close {} > Upper Band {}", close, upper.round_dp(2)),
                            timestamp_ms: ts,
                        });
                    }
                }
            }
        }

        // Slide window
        for i in window_size..close_arr.len() {
            if let Some(old_val) = close_arr.get(i - window_size) {
                if let Some(old_d) = Decimal::from_f64_retain(old_val) {
                    current_sum -= old_d;
                    current_sum_sq -= old_d * old_d;
                }
            }

            if let Some(new_val) = close_arr.get(i) {
                if let Some(new_d) = Decimal::from_f64_retain(new_val) {
                    current_sum += new_d;
                    current_sum_sq += new_d * new_d;

                    let mean = current_sum / window_size_dec;
                    let variance = (current_sum_sq / window_size_dec) - (mean * mean);
                    let std_dev = if variance <= Decimal::ZERO {
                        Decimal::ZERO
                    } else {
                        variance.sqrt().unwrap_or(Decimal::ZERO)
                    };

                    let upper = mean + (std_dev * num_std_dev_dec);
                    let lower = mean - (std_dev * num_std_dev_dec);

                    let prev_close = close_arr
                        .get(i - 1)
                        .and_then(|v| Decimal::from_f64_retain(v))
                        .unwrap_or(prev_mean); // Fallback to mean if prev missing (shouldn't happen)

                    if let Some(ts) = time_arr.get(i) {
                        // Entry / ScaleIn Logic
                        if new_d < lower {
                            let signal_type = if new_d < lower - (half_dec * std_dev) {
                                SignalType::ScaleIn
                            } else {
                                SignalType::Entry
                            };

                            let sl = new_d - (two_dec * std_dev);
                            let tp = mean;

                            signals.push(Signal {
                                signal_type,
                                symbol: self.config.symbol.clone(),
                                side: "buy".to_string(),
                                size_hint: "100".to_string(),
                                confidence: 0.8,
                                stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                                take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                                reason: format!(
                                    "Close {} < Lower Band {}",
                                    new_d,
                                    lower.round_dp(2)
                                ),
                                timestamp_ms: ts,
                            });
                        } else if new_d > upper {
                            let signal_type = if new_d > upper + (half_dec * std_dev) {
                                SignalType::ScaleIn
                            } else {
                                SignalType::Entry
                            };

                            let sl = new_d + (two_dec * std_dev);
                            let tp = mean;

                            signals.push(Signal {
                                signal_type,
                                symbol: self.config.symbol.clone(),
                                side: "sell".to_string(),
                                size_hint: "100".to_string(),
                                confidence: 0.8,
                                stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                                take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                                reason: format!(
                                    "Close {} > Upper Band {}",
                                    new_d,
                                    upper.round_dp(2)
                                ),
                                timestamp_ms: ts,
                            });
                        }

                        // Exit Logic (Mean Crossover)
                        // Exit Long (Sell): Price crosses SMA from below
                        // if prev_close < prev_mean && new_d >= mean
                        if prev_close < prev_mean && new_d >= mean {
                            signals.push(Signal {
                                signal_type: SignalType::Exit,
                                symbol: self.config.symbol.clone(),
                                side: "sell".to_string(), // Exit Long
                                size_hint: "max".to_string(),
                                confidence: 0.6,
                                stop_loss: None,
                                take_profit: None,
                                reason: format!(
                                    "Price crossed SMA from below ({} -> {}, Mean {})",
                                    prev_close,
                                    new_d,
                                    mean.round_dp(2)
                                ),
                                timestamp_ms: ts,
                            });
                        }

                        // Exit Short (Buy): Price crosses SMA from above
                        // if prev_close > prev_mean && new_d <= mean
                        if prev_close > prev_mean && new_d <= mean {
                            signals.push(Signal {
                                signal_type: SignalType::Exit,
                                symbol: self.config.symbol.clone(),
                                side: "buy".to_string(), // Exit Short
                                size_hint: "max".to_string(),
                                confidence: 0.6,
                                stop_loss: None,
                                take_profit: None,
                                reason: format!(
                                    "Price crossed SMA from above ({} -> {}, Mean {})",
                                    prev_close,
                                    new_d,
                                    mean.round_dp(2)
                                ),
                                timestamp_ms: ts,
                            });
                        }
                    }

                    prev_mean = mean;
                }
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
        assert!(last_signal.stop_loss.is_some());
        assert!(last_signal.take_profit.is_some());

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
        assert!(s.stop_loss.is_none());
        assert!(s.take_profit.is_none());

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
