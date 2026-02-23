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

        // Check first window
        {
            let i = window_size - 1;
            let mean = current_sum / window_size as f64;
            let variance = (current_sum_sq / window_size as f64) - (mean * mean);
            let std_dev = if variance < 0.0 { 0.0 } else { variance.sqrt() };
            let upper = mean + (std_dev * self.config.num_std_dev);
            let lower = mean - (std_dev * self.config.num_std_dev);

            if let (Some(close), Some(ts)) = (close_arr.get(i), time_arr.get(i)) {
                if close < lower {
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        reason: format!("Close {:.2} < Lower Band {:.2}", close, lower),
                        timestamp_ms: ts,
                    });
                } else if close > upper {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
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

                if let Some(ts) = time_arr.get(i) {
                    if new_val < lower {
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            reason: format!("Close {:.2} < Lower Band {:.2}", new_val, lower),
                            timestamp_ms: ts,
                        });
                    } else if new_val > upper {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            reason: format!("Close {:.2} > Upper Band {:.2}", new_val, upper),
                            timestamp_ms: ts,
                        });
                    }
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
        // then 15 (mean ~11, std increase) -> 15 > upper -> Sell Signal
        let df = df! (
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "close" => &[10.0, 10.0, 10.0, 15.0],
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Window 3.
        // Index 2 (3000): vals [10, 10, 10]. Mean 10. Std 0. Upper 10. Lower 10. Close 10. No signal (edge case) or maybe?
        // Index 3 (4000): vals [10, 10, 15]. Mean 11.66. Std ~2.3. Upper 14. Lower 9. Close 15. 15 > 14 -> Exit/Sell.

        // Actually my manual calculation:
        // [10, 10, 15]. Sum 35. Mean 11.666.
        // SumSq 100+100+225 = 425.
        // Var = 425/3 - (35/3)^2 = 141.66 - 136.11 = 5.55.
        // Std = 2.35.
        // Upper = 11.66 + 2.35 = 14.01.
        // Close 15 > 14.01 -> Signal.

        assert!(!signals.is_empty(), "Should generate signals");
        let last_signal = signals.last().unwrap();
        assert_eq!(last_signal.signal_type, SignalType::Exit);
        assert_eq!(last_signal.side, "sell");

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
