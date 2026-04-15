//! The Zero Lag Exponential Moving Average (ZLEMA) Crossover Strategy
//!
//! ZLEMA is designed to eliminate the inherent lag associated with all trend-following indicators.
//! This strategy uses a fast ZLEMA and a slow ZLEMA to generate highly responsive trend-following signals.
//!
//! - **Entry Signal:** A buy signal is generated when the fast ZLEMA crosses above the slow ZLEMA.
//! - **Exit Signal:** A sell signal is generated when the fast ZLEMA crosses below the slow ZLEMA.
//!
use crate::indicators::{atr, zlema};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration parameters for the `ZlemaCrossover` strategy.
///
/// # Examples
///
/// ```
/// use strategies::zlema_crossover::ZlemaCrossoverConfig;
///
/// let config = ZlemaCrossoverConfig {
///     short_period: 9,
///     long_period: 21,
///     stop_loss_atr_mult: 2.0,
///     take_profit_atr_mult: 2.0,
///     max_position_size: 100.0,
///     atr_period: 14,
///     symbol: "BTCUSD".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZlemaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub take_profit_atr_mult: f64,
    pub max_position_size: f64,
    pub symbol: String,
}

impl Default for ZlemaCrossoverConfig {
    fn default() -> Self {
        Self {
            short_period: 9,
            long_period: 21,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            take_profit_atr_mult: 4.0,
            max_position_size: 1.0,
            symbol: "BTCUSD".to_string(),
        }
    }
}

impl StrategyConfig for ZlemaCrossoverConfig {}

/// The ZLEMA Crossover strategy implementation.
///
/// # Examples
///
/// ```
/// use strategies::zlema_crossover::{ZlemaCrossover, ZlemaCrossoverConfig};
/// use strategies::strategy::Strategy;
///
/// let config = ZlemaCrossoverConfig {
///     short_period: 9,
///     long_period: 21,
///     stop_loss_atr_mult: 2.0,
///     take_profit_atr_mult: 2.0,
///     max_position_size: 100.0,
///     atr_period: 14,
///     symbol: "BTCUSD".to_string(),
/// };
///
/// let strategy = ZlemaCrossover::new(config);
/// assert_eq!(strategy.name(), "ZlemaCrossover");
/// ```
pub struct ZlemaCrossover {
    config: ZlemaCrossoverConfig,
}

impl ZlemaCrossover {
    pub fn new(config: ZlemaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for ZlemaCrossover {
    fn name(&self) -> &str {
        "ZlemaCrossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let max_period = std::cmp::max(self.config.short_period, self.config.long_period);
        let max_lookback = std::cmp::max(max_period, self.config.atr_period);

        if data.height() < max_lookback + 1 {
            return Ok(vec![]);
        }

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let close_series = data.column("close")?.f64()?;

        let short_zlema_series = zlema::calculate(data, self.config.short_period)?;
        let long_zlema_series = zlema::calculate(data, self.config.long_period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let short_zlema_arr = short_zlema_series.f64()?;
        let long_zlema_arr = long_zlema_series.f64()?;
        let atr_arr = atr_series.f64()?;

        let short_zlema_vec: Vec<Option<f64>> = short_zlema_arr.into_iter().collect();
        let long_zlema_vec: Vec<Option<f64>> = long_zlema_arr.into_iter().collect();
        let atr_vec: Vec<Option<f64>> = atr_arr.into_iter().collect();

        let mut signals = Vec::new();
        let len = data.height();

        for i in max_lookback..len {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price = close_series.get(i).unwrap_or(0.0);

            // Current bar
            let short_zlema = short_zlema_vec.get(i).copied().flatten();
            let long_zlema = long_zlema_vec.get(i).copied().flatten();

            // Previous bar
            let prev_short_zlema = short_zlema_vec.get(i - 1).copied().flatten();
            let prev_long_zlema = long_zlema_vec.get(i - 1).copied().flatten();

            let atr_val_opt = atr_vec.get(i).copied().flatten();

            if let (
                Some(curr_short),
                Some(curr_long),
                Some(prev_short),
                Some(prev_long),
                Some(atr_val),
            ) = (
                short_zlema,
                long_zlema,
                prev_short_zlema,
                prev_long_zlema,
                atr_val_opt,
            ) {
                let stop_loss_dist = atr_val * self.config.stop_loss_atr_mult;
                let take_profit_dist = atr_val * self.config.take_profit_atr_mult;

                // Golden Cross (Entry Long)
                if prev_short <= prev_long && curr_short > curr_long {
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(price - stop_loss_dist),
                        take_profit: Some(price + take_profit_dist),
                        reason: "ZLEMA Golden Cross".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Death Cross (Exit Long)
                if prev_short >= prev_long && curr_short < curr_long {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "ZLEMA Death Cross".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: ZlemaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_zlema_crossover_entry_and_exit() -> Result<()> {
        let config = ZlemaCrossoverConfig {
            short_period: 2,
            long_period: 4,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            take_profit_atr_mult: 4.0,
            max_position_size: 1.0,
            symbol: "TEST".to_string(),
        };
        let strategy = ZlemaCrossover::new(config);

        let mut closes = vec![100.0; 20];
        // Create an uptrend to trigger golden cross
        closes[10] = 101.0;
        closes[11] = 105.0;
        closes[12] = 110.0;
        closes[13] = 115.0;
        // Create a downtrend to trigger death cross
        closes[14] = 110.0;
        closes[15] = 100.0;
        closes[16] = 90.0;
        closes[17] = 80.0;
        closes[18] = 70.0;

        let timestamps: Vec<i64> = (0..20).map(|i| 1000 + i as i64 * 1000).collect();
        let highs = closes.iter().map(|c| c + 2.0).collect::<Vec<_>>();
        let lows = closes.iter().map(|c| c - 2.0).collect::<Vec<_>>();

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "open" => &closes,
            "high" => &highs,
            "low" => &lows,
            "close" => &closes,
            "volume" => vec![1000.0; 20]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();

        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit && s.side == "sell")
            .collect();

        assert!(!entries.is_empty(), "Should generate an entry signal");
        assert!(!exits.is_empty(), "Should generate an exit signal");
        assert!(entries[0].reason.contains("Golden Cross"));
        assert!(exits[0].reason.contains("Death Cross"));
        assert!(entries[0].stop_loss.is_some());
        assert!(entries[0].take_profit.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_zlema_crossover_empty_data() -> Result<()> {
        let config = ZlemaCrossoverConfig::default();
        let strategy = ZlemaCrossover::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[] as &[i64],
            "open" => &[] as &[f64],
            "high" => &[] as &[f64],
            "low" => &[] as &[f64],
            "close" => &[] as &[f64],
            "volume" => &[] as &[f64]
        )?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(
            signals.is_empty(),
            "Empty dataframe should produce no signals"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_zlema_crossover_extreme_volatility() -> Result<()> {
        let config = ZlemaCrossoverConfig {
            short_period: 2,
            long_period: 4,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            take_profit_atr_mult: 4.0,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        };
        let strategy = ZlemaCrossover::new(config);

        let mut closes = vec![100.0; 20];
        closes[10] = 101.0;
        closes[11] = 105.0;
        closes[12] = 1000.0; // Extreme spike up
        closes[13] = 115.0;
        closes[14] = 110.0;
        closes[15] = 10.0; // Extreme spike down
        closes[16] = 90.0;

        let timestamps: Vec<i64> = (0..20).map(|i| 1000 + i as i64 * 1000).collect();
        // High ATR from massive spikes
        let highs = closes.iter().map(|c| c + 100.0).collect::<Vec<_>>();
        let lows = closes
            .iter()
            .map(|c| if *c > 100.0 { c - 100.0 } else { 0.0 })
            .collect::<Vec<_>>();

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "open" => &closes,
            "high" => &highs,
            "low" => &lows,
            "close" => &closes,
            "volume" => vec![1000.0; 20]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Strategy should still process it and generate signals without panicking
        // Depending on indicator logic, extreme jumps might cause an immediate cross
        // The main test is that it doesn't crash on wild data
        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();

        // We expect at least one signal around the spikes
        assert!(!entries.is_empty() || !signals.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_zlema_parameter_validation() {
        let config = ZlemaCrossoverConfig::default();
        let mut strategy = ZlemaCrossover::new(config.clone());

        assert_eq!(strategy.name(), "ZlemaCrossover");
        assert_eq!(strategy.strategy_type(), StrategyType::TrendFollowing);

        let new_params = serde_json::json!({
            "short_period": 5,
            "long_period": 10,
            "atr_period": 14,
            "stop_loss_atr_mult": 1.5,
            "take_profit_atr_mult": 3.0,
            "max_position_size": 2.5,
            "symbol": "ETHUSD"
        });

        strategy.update_params(new_params).await.unwrap();
        assert_eq!(strategy.config.short_period, 5);
        assert_eq!(strategy.config.long_period, 10);
        assert_eq!(strategy.config.stop_loss_atr_mult, 1.5);
        assert_eq!(strategy.config.max_position_size, 2.5);
    }
}
