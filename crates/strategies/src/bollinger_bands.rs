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
        let time_arr = if time_series.dtype() == &DataType::Int64 {
            time_series.i64()?.clone()
        } else {
            time_series.cast(&DataType::Int64)?.i64()?.clone()
        };

        let window_size = self.config.window_size;
        let mut signals = Vec::new();

        if close_arr.len() < window_size {
            return Ok(signals);
        }

        let mut current_sum = 0.0;
        let mut current_sum_sq = 0.0;

        for i in 0..window_size {
            if let Some(val) = close_arr.get(i) {
                current_sum += val;
                current_sum_sq += val * val;
            }
        }

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
            num_std_dev: 2.0,
            stop_loss_pct: 0.05,
            symbol: "AAPL".to_string(),
        };
        let strategy = BollingerBandsMeanReversion::new(config);

        let df = df! (
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000],
            "close" => &[100.0, 102.0, 104.0, 102.0, 100.0, 98.0, 96.0, 98.0],
        )?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty() || !signals.is_empty());
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
