use crate::indicators::{atr, macd};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Configuration parameters for the `MacdTrendFollower` strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacdTrendFollowerConfig {
    pub fast_period: usize,
    pub slow_period: usize,
    pub signal_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for MacdTrendFollowerConfig {
    fn default() -> Self {
        Self {
            fast_period: 12,
            slow_period: 26,
            signal_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "BTCUSD".to_string(),
        }
    }
}

impl StrategyConfig for MacdTrendFollowerConfig {}

/// A trend-following strategy using the MACD indicator.
pub struct MacdTrendFollower {
    config: MacdTrendFollowerConfig,
}

impl MacdTrendFollower {
    pub fn new(config: MacdTrendFollowerConfig) -> Result<Self> {
        if config.fast_period >= config.slow_period {
            anyhow::bail!("fast_period must be less than slow_period");
        }
        if config.fast_period == 0
            || config.slow_period == 0
            || config.signal_period == 0
            || config.atr_period == 0
        {
            anyhow::bail!("All periods must be greater than 0");
        }
        Ok(Self { config })
    }
}

#[async_trait]
impl Strategy for MacdTrendFollower {
    fn name(&self) -> &str {
        "MacdTrendFollower"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let (macd_line_series, signal_line_series, _histogram) = macd::calculate(
            data,
            self.config.fast_period,
            self.config.slow_period,
            self.config.signal_period,
        )?;
        let macd_line_arr = macd_line_series.f64()?;
        let signal_line_arr = signal_line_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        if close_arr.len() < 2 {
            return Ok(signals);
        }

        // Generate signal for all points in history to ensure properly backtestable logic
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let macd_curr = macd_line_arr.get(i);
            let macd_prev = macd_line_arr.get(i - 1);
            let sig_curr = signal_line_arr.get(i);
            let sig_prev = signal_line_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(macd_c),
                Some(macd_p),
                Some(sig_c),
                Some(sig_p),
                Some(price),
                Some(atr_val),
            ) = (macd_curr, macd_prev, sig_curr, sig_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);
                let sl_mult = Decimal::from_f64_retain(self.config.stop_loss_atr_mult)
                    .unwrap_or(Decimal::ZERO);
                let two_dec = Decimal::from(2);

                // Long Entry: MACD Line crosses above Signal Line.
                if macd_p <= sig_p && macd_c > sig_c {
                    // Exit any active short position
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short (buy to cover)
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "MACD Crossover Up: MACD {:.2} > Signal {:.2}",
                            macd_c, sig_c
                        ),
                        timestamp_ms: timestamp,
                    });

                    // Enter Long
                    let sl = price_dec - (atr_dec * sl_mult);
                    let risk = price_dec - sl;
                    let tp = price_dec + (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "MACD Crossover Up: MACD {:.2} > Signal {:.2}",
                            macd_c, sig_c
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Short Entry: MACD Line crosses below Signal Line.
                else if macd_p >= sig_p && macd_c < sig_c {
                    // Exit any active long position
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long (sell to close)
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "MACD Crossover Down: MACD {:.2} < Signal {:.2}",
                            macd_c, sig_c
                        ),
                        timestamp_ms: timestamp,
                    });

                    // Enter Short
                    let sl = price_dec + (atr_dec * sl_mult);
                    let risk = sl - price_dec;
                    let tp = price_dec - (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "MACD Crossover Down: MACD {:.2} < Signal {:.2}",
                            macd_c, sig_c
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: MacdTrendFollowerConfig = serde_json::from_value(params)?;
        if new_config.fast_period >= new_config.slow_period {
            anyhow::bail!("fast_period must be less than slow_period");
        }
        if new_config.fast_period == 0
            || new_config.slow_period == 0
            || new_config.signal_period == 0
            || new_config.atr_period == 0
        {
            anyhow::bail!("All periods must be greater than 0");
        }
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_macd_trend_follower_signals() -> Result<()> {
        let config = MacdTrendFollowerConfig {
            fast_period: 2,
            slow_period: 4,
            signal_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = MacdTrendFollower::new(config)?;

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000],
            "open"  => &[10.0, 15.0, 12.0, 18.0, 20.0, 25.0, 30.0, 15.0, 10.0],
            "high"  => &[10.0, 15.0, 12.0, 18.0, 20.0, 25.0, 30.0, 15.0, 10.0],
            "low"   => &[ 5.0,  8.0,  6.0, 10.0, 15.0, 20.0, 25.0, 12.0,  8.0],
            "close" => &[10.0, 15.0, 12.0, 18.0, 20.0, 25.0, 30.0, 15.0, 10.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // We expect some exit and entry signals since the data goes up strongly then down strongly
        let exits = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .count();
        let entries = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .count();

        assert!(exits > 0, "Expected at least one exit signal");
        assert!(entries > 0, "Expected at least one entry signal");
        assert_eq!(
            exits, entries,
            "Expected exits and entries to match due to crossover reversals"
        );

        Ok(())
    }

    #[test]
    fn test_parameter_validation() {
        // Fast period >= slow period
        let config = MacdTrendFollowerConfig {
            fast_period: 26,
            slow_period: 12,
            signal_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        assert!(MacdTrendFollower::new(config).is_err());

        // Zero period
        let config = MacdTrendFollowerConfig {
            fast_period: 12,
            slow_period: 26,
            signal_period: 0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        assert!(MacdTrendFollower::new(config).is_err());
    }

    #[tokio::test]
    async fn test_parameter_update() -> Result<()> {
        let mut strategy = MacdTrendFollower::new(MacdTrendFollowerConfig {
            fast_period: 12,
            slow_period: 26,
            signal_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        })?;

        let new_params = serde_json::json!({
            "fast_period": 10,
            "slow_period": 20,
            "signal_period": 8,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 20,
            "symbol": "NEW_TEST"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.fast_period, 10);
        assert_eq!(strategy.config.symbol, "NEW_TEST");

        let bad_params = serde_json::json!({
            "fast_period": 30,
            "slow_period": 20,
            "signal_period": 8,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 20,
            "symbol": "NEW_TEST"
        });

        assert!(strategy.update_params(bad_params).await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_edge_cases() -> Result<()> {
        let config = MacdTrendFollowerConfig {
            fast_period: 2,
            slow_period: 4,
            signal_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = MacdTrendFollower::new(config)?;

        // Empty data
        let empty_df = DataFrame::default();
        let res_empty = strategy.generate_signals(&empty_df).await;
        assert!(res_empty.is_err() || res_empty.unwrap().is_empty());

        // 1 point data
        let df_1pt = df!(
            "timestamp_unix_ms" => &[1000i64],
            "open"  => &[10.0],
            "high"  => &[10.0],
            "low"   => &[ 5.0],
            "close" => &[10.0]
        )?;
        let res_1pt = strategy.generate_signals(&df_1pt).await?;
        assert!(res_1pt.is_empty());

        Ok(())
    }
}
