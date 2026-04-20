//! The Weighted Moving Average (SMMA) Crossover Strategy
//!
//! The Weighted Moving Average assigns more weight to recent data points, making it more responsive to price changes than a Simple Moving Average.
//! This strategy uses a fast SMMA and a slow SMMA to generate trend-following signals.
//!
//! - **Entry Signal:** A buy signal is generated when the fast SMMA crosses above the slow SMMA.
//! - **Exit Signal:** A sell signal is generated when the fast SMMA crosses below the slow SMMA.
//!
use crate::indicators::{atr, smma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

/// The SMMA Crossover strategy implementation.
///
/// # Examples
///
/// ```
/// use strategies::smma_crossover::{SmmaCrossover, SmmaCrossoverConfig};
/// use strategies::strategy::Strategy;
///
/// let config = SmmaCrossoverConfig {
///     short_window: 9,
///     long_window: 21,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     max_position_size: 100.0,
///     symbol: "BTCUSD".to_string(),
/// };
///
/// let strategy = SmmaCrossover::new(config);
/// assert_eq!(strategy.name(), "SmmaCrossover");
/// ```
pub struct SmmaCrossover {
    config: SmmaCrossoverConfig,
}

/// Configuration parameters for the `SmmaCrossover` strategy.
///
/// # Examples
///
/// ```
/// use strategies::smma_crossover::SmmaCrossoverConfig;
///
/// let config = SmmaCrossoverConfig {
///     short_window: 9,
///     long_window: 21,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     max_position_size: 100.0,
///     symbol: "BTCUSD".to_string(),
/// };
/// ```
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SmmaCrossoverConfig {
    pub max_position_size: f64,
    pub short_window: usize,
    pub long_window: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for SmmaCrossoverConfig {}

impl SmmaCrossover {
    pub fn new(config: SmmaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for SmmaCrossover {
    fn name(&self) -> &str {
        "SmmaCrossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if self.config.short_window == 0 || self.config.long_window == 0 {
            anyhow::bail!("Window periods must be > 0");
        }
        if self.config.short_window >= self.config.long_window {
            anyhow::bail!("Short window must be less than long window");
        }
        if self.config.max_position_size <= 0.0 {
            anyhow::bail!("Max position size must be > 0");
        }

        if data.height() == 0 {
            return Ok(vec![]);
        }

        let short_smma = smma::calculate(data, self.config.short_window)?;
        let long_smma = smma::calculate(data, self.config.long_window)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let short_smma_f64 = short_smma.f64()?;
        let long_smma_f64 = long_smma.f64()?;
        let atr_f64 = atr_series.f64()?;

        let close_series = data.column("close")?.f64()?;
        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let mut in_position = false;
        let mut current_stop_loss = 0.0;

        for i in 1..data.height() {
            let current_close = close_series.get(i);
            let current_short_smma = short_smma_f64.get(i);
            let current_long_smma = long_smma_f64.get(i);
            let prev_short_smma = short_smma_f64.get(i - 1);
            let prev_long_smma = long_smma_f64.get(i - 1);
            let current_atr = atr_f64.get(i);
            let timestamp = timestamps.get(i).unwrap_or(0);

            if let (
                Some(c),
                Some(curr_s_smma),
                Some(curr_l_smma),
                Some(prev_s_smma),
                Some(prev_l_smma),
                Some(atr),
            ) = (
                current_close,
                current_short_smma,
                current_long_smma,
                prev_short_smma,
                prev_long_smma,
                current_atr,
            ) {
                if !in_position {
                    // Check for Entry (Short crosses above Long SMMA)
                    if prev_s_smma <= prev_l_smma && curr_s_smma > curr_l_smma {
                        in_position = true;
                        current_stop_loss = c - (atr * self.config.stop_loss_atr_mult);

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: format!("{}", self.config.max_position_size),
                            confidence: 0.8,
                            stop_loss: Some(current_stop_loss),
                            take_profit: None,
                            reason: "Short SMMA crossed above Long SMMA".to_string(),
                            timestamp_ms: timestamp,
                        });
                    }
                } else {
                    // Check for Exit (Stop loss hit or Short crosses below Long SMMA)
                    let mut exit_reason = None;

                    if c <= current_stop_loss {
                        exit_reason = Some("Stop Loss Hit".to_string());
                    } else if prev_s_smma >= prev_l_smma && curr_s_smma < curr_l_smma {
                        exit_reason = Some("Short SMMA crossed below Long SMMA".to_string());
                    }

                    // Update stop loss trailing (if price moves in our favor)
                    let new_stop = c - (atr * self.config.stop_loss_atr_mult);
                    if new_stop > current_stop_loss {
                        current_stop_loss = new_stop;
                    }

                    if let Some(reason) = exit_reason {
                        in_position = false;
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason,
                            timestamp_ms: timestamp,
                        });
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: SmmaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        // Needs enough data for SMMA(20) and ATR(14)
        let mut close_vals = vec![100.0; 50];
        let mut high_vals = vec![101.0; 50];
        let mut low_vals = vec![99.0; 50];
        let timestamps: Vec<i64> = (0..50).map(|i| i as i64 * 1000).collect();

        // Create a trend down to establish Short SMMA < Long SMMA
        for i in 10..30 {
            close_vals[i] = 100.0 - (i as f64 - 10.0);
            high_vals[i] = close_vals[i] + 1.0;
            low_vals[i] = close_vals[i] - 1.0;
        }
        // Create a trend up to force Short SMMA to cross above Long SMMA
        for i in 30..50 {
            close_vals[i] = 80.0 + (i as f64 - 30.0) * 3.0;
            high_vals[i] = close_vals[i] + 1.0;
            low_vals[i] = close_vals[i] - 1.0;
        }

        df!(
            "timestamp_unix_ms" => timestamps,
            "open" => close_vals.clone(),
            "high" => high_vals,
            "low" => low_vals,
            "close" => close_vals,
            "volume" => vec![1000.0; 50]
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_smma_crossover_empty_data() -> Result<()> {
        let config = SmmaCrossoverConfig {
            short_window: 9,
            long_window: 21,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        };

        let strategy = SmmaCrossover::new(config);
        let empty_df = DataFrame::empty();
        let signals = strategy.generate_signals(&empty_df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_smma_crossover_signal_generation() -> Result<()> {
        let config = SmmaCrossoverConfig {
            short_window: 5,
            long_window: 10,
            stop_loss_atr_mult: 1.0,
            atr_period: 5,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        };

        let strategy = SmmaCrossover::new(config);
        let df = create_test_data();
        let signals = strategy.generate_signals(&df).await?;

        // We expect at least one entry signal because of the designed V-shape recovery
        assert!(!signals.is_empty(), "Should generate signals");

        let entry_signal = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry_signal.is_some(), "Should have an entry signal");
        if let Some(s) = entry_signal {
            assert_eq!(s.side, "buy");
            assert!(s.stop_loss.is_some());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_smma_crossover_parameter_validation() -> Result<()> {
        let mut strategy = SmmaCrossover::new(SmmaCrossoverConfig {
            short_window: 9,
            long_window: 21,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "short_window": 5,
            "long_window": 10,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 5,
            "max_position_size": 200.0,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.short_window, 5);
        assert_eq!(strategy.config.long_window, 10);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}

#[cfg(test)]
mod tests_invalid_params {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        let close_vals = vec![100.0; 50];
        let high_vals = vec![101.0; 50];
        let low_vals = vec![99.0; 50];
        let timestamps: Vec<i64> = (0..50).map(|i| i as i64 * 1000).collect();

        df!(
            "timestamp_unix_ms" => timestamps,
            "open" => close_vals.clone(),
            "high" => high_vals,
            "low" => low_vals,
            "close" => close_vals,
            "volume" => vec![1000.0; 50]
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_smma_crossover_invalid_params() -> Result<()> {
        let config = SmmaCrossoverConfig {
            short_window: 10,
            long_window: 5,
            stop_loss_atr_mult: 1.0,
            atr_period: 5,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        };

        let strategy = SmmaCrossover::new(config);
        let df = create_test_data();
        let signals = strategy.generate_signals(&df).await;
        assert!(signals.is_err());

        let config2 = SmmaCrossoverConfig {
            short_window: 0,
            long_window: 5,
            stop_loss_atr_mult: 1.0,
            atr_period: 5,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        };

        let strategy2 = SmmaCrossover::new(config2);
        let signals2 = strategy2.generate_signals(&df).await;
        assert!(signals2.is_err());

        Ok(())
    }
}
