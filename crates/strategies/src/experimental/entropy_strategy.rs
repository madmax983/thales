use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyStrategyConfig {
    pub window_size: usize,
    pub num_bins: usize,
    pub entropy_threshold: f64,
    pub stop_loss_pct: f64,
    pub symbol: String,
}

impl StrategyConfig for EntropyStrategyConfig {}

pub struct EntropyStrategy {
    pub config: EntropyStrategyConfig,
}

impl EntropyStrategy {
    pub fn new(config: EntropyStrategyConfig) -> Self {
        Self { config }
    }

    fn calculate_entropy(returns: &[f64], num_bins: usize) -> f64 {
        if returns.len() < 2 || num_bins == 0 {
            return 0.0;
        }

        let mut min_ret = f64::MAX;
        let mut max_ret = f64::MIN;

        for &r in returns {
            if r < min_ret {
                min_ret = r;
            }
            if r > max_ret {
                max_ret = r;
            }
        }

        if max_ret == min_ret {
            return 0.0;
        }

        let bin_size = (max_ret - min_ret) / num_bins as f64;
        let mut bin_counts = vec![0; num_bins];

        for &r in returns {
            let mut bin_idx = ((r - min_ret) / bin_size).floor() as usize;
            if bin_idx >= num_bins {
                bin_idx = num_bins - 1;
            }
            bin_counts[bin_idx] += 1;
        }

        let total_returns = returns.len() as f64;
        let mut entropy = 0.0;

        for count in bin_counts {
            let prob = count as f64 / total_returns;
            if prob > 0.0 {
                entropy -= prob * prob.log2();
            }
        }

        let max_entropy = (num_bins as f64).log2();
        if max_entropy > 0.0 {
            entropy / max_entropy
        } else {
            0.0
        }
    }
}

#[async_trait]
impl Strategy for EntropyStrategy {
    fn name(&self) -> &str {
        "EntropyStrategy"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let mut signals = Vec::new();

        if data.height() < self.config.window_size + 1 {
            return Ok(signals);
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let mut returns = Vec::with_capacity(close_arr.len() - 1);
        for i in 1..close_arr.len() {
            if let (Some(prev), Some(curr)) = (close_arr.get(i - 1), close_arr.get(i)) {
                if prev > 0.0 {
                    returns.push((curr / prev).ln());
                } else {
                    returns.push(0.0);
                }
            } else {
                returns.push(0.0);
            }
        }

        let mut in_position = false;
        let mut position_side = "";

        for i in self.config.window_size..returns.len() {
            let window_returns = &returns[i - self.config.window_size..i];
            let normalized_entropy = Self::calculate_entropy(window_returns, self.config.num_bins);
            let current_return = window_returns.last().unwrap_or(&0.0);

            let timestamp = time_arr.get(i + 1).unwrap_or(0);
            let current_close = close_arr.get(i + 1).unwrap_or(0.0);

            if current_close == 0.0 {
                continue;
            }

            if !in_position {
                if normalized_entropy < self.config.entropy_threshold {
                    if *current_return > 0.0 {
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(current_close * (1.0 - self.config.stop_loss_pct)),
                            take_profit: None,
                            reason: format!(
                                "Low Entropy ({:.2}) + Positive Momentum",
                                normalized_entropy
                            ),
                            timestamp_ms: timestamp,
                        });
                        in_position = true;
                        position_side = "buy";
                    } else if *current_return < 0.0 {
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(current_close * (1.0 + self.config.stop_loss_pct)),
                            take_profit: None,
                            reason: format!(
                                "Low Entropy ({:.2}) + Negative Momentum",
                                normalized_entropy
                            ),
                            timestamp_ms: timestamp,
                        });
                        in_position = true;
                        position_side = "sell";
                    }
                }
            } else {
                if normalized_entropy > self.config.entropy_threshold {
                    let exit_side = if position_side == "buy" {
                        "sell"
                    } else {
                        "buy"
                    };
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: exit_side.to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.9,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("High Entropy ({:.2}) -> Exiting", normalized_entropy),
                        timestamp_ms: timestamp,
                    });
                    in_position = false;
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: EntropyStrategyConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_entropy_strategy_signals() -> Result<()> {
        let config = EntropyStrategyConfig {
            window_size: 3,
            num_bins: 2,
            entropy_threshold: 0.8,
            stop_loss_pct: 0.05,
            symbol: "TEST".to_string(),
        };
        let strategy = EntropyStrategy::new(config);

        let timestamps = vec![1000i64, 2000, 3000, 4000, 5000, 6000];
        let closes = vec![100.0, 105.0, 110.25, 115.7625, 100.0, 120.0];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await;
        assert!(signals.is_ok());
        let signals = signals.unwrap();
        assert!(!signals.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_entropy_strategy_empty_data() -> Result<()> {
        let config = EntropyStrategyConfig {
            window_size: 3,
            num_bins: 2,
            entropy_threshold: 0.8,
            stop_loss_pct: 0.05,
            symbol: "TEST".to_string(),
        };
        let strategy = EntropyStrategy::new(config);

        let df = df!(
            "timestamp_unix_ms" => Vec::<i64>::new(),
            "close" => Vec::<f64>::new()
        )?;

        let signals = strategy.generate_signals(&df).await;
        assert!(signals.is_ok());
        assert!(signals.unwrap().is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let config = EntropyStrategyConfig {
            window_size: 20,
            num_bins: 10,
            entropy_threshold: 0.5,
            stop_loss_pct: 0.05,
            symbol: "AAPL".to_string(),
        };
        let mut strategy = EntropyStrategy::new(config);

        let new_params = serde_json::json!({
            "window_size": 30,
            "num_bins": 10,
            "entropy_threshold": 0.5,
            "stop_loss_pct": 0.05,
            "symbol": "AAPL"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.window_size, 30);

        let invalid_params = serde_json::json!({
            "invalid_field": "test"
        });
        assert!(strategy.update_params(invalid_params).await.is_err());

        Ok(())
    }
}
