//! The PPO + RSI Strategy
//!
//! Combines the Percentage Price Oscillator with RSI.
//!
use crate::indicators::{atr, ppo, rsi};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::{Context, Result};
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PpoRsiTrendConfig {
    pub ppo_fast_period: usize,
    pub ppo_slow_period: usize,
    pub ppo_signal_period: usize,
    pub rsi_period: usize,
    pub rsi_buy_threshold: f64,
    pub rsi_sell_threshold: f64,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}

impl StrategyConfig for PpoRsiTrendConfig {}

impl PpoRsiTrendConfig {
    pub fn validate(&self) -> Result<()> {
        if self.ppo_fast_period >= self.ppo_slow_period {
            anyhow::bail!("ppo_fast_period must be less than ppo_slow_period");
        }
        if self.ppo_fast_period == 0 || self.ppo_slow_period == 0 || self.ppo_signal_period == 0 {
            anyhow::bail!("PPO periods must be greater than 0");
        }
        if self.rsi_period == 0 {
            anyhow::bail!("rsi_period must be greater than 0");
        }
        if self.atr_period == 0 {
            anyhow::bail!("atr_period must be greater than 0");
        }
        if self.rsi_buy_threshold < 0.0 || self.rsi_buy_threshold > 100.0 {
            anyhow::bail!("rsi_buy_threshold must be between 0 and 100");
        }
        if self.rsi_sell_threshold < 0.0 || self.rsi_sell_threshold > 100.0 {
            anyhow::bail!("rsi_sell_threshold must be between 0 and 100");
        }
        Ok(())
    }
}

pub struct PpoRsiTrend {
    config: PpoRsiTrendConfig,
}

impl PpoRsiTrend {
    pub fn new(config: PpoRsiTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for PpoRsiTrend {
    fn name(&self) -> &str {
        "PpoRsiTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        self.config.validate()?;

        if data.height() == 0 {
            return Ok(Vec::new());
        }

        let close_series = data
            .column("close")
            .context("Missing 'close' column")?
            .clone();
        let close_arr = close_series.f64()?;

        let time_series = data
            .column("timestamp_unix_ms")
            .context("Missing 'timestamp_unix_ms' column")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let (ppo_line_series, ppo_signal_series, _) = ppo::calculate(
            data,
            self.config.ppo_fast_period,
            self.config.ppo_slow_period,
            self.config.ppo_signal_period,
        )?;

        let rsi_series = rsi::calculate(data, self.config.rsi_period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let ppo_line_arr = ppo_line_series.f64()?;
        let ppo_signal_arr = ppo_signal_series.f64()?;
        let rsi_arr = rsi_series.f64()?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let p_line_c = ppo_line_arr.get(i);
            let p_line_p = ppo_line_arr.get(i - 1);
            let p_sig_c = ppo_signal_arr.get(i);
            let p_sig_p = ppo_signal_arr.get(i - 1);
            let rsi_c = rsi_arr.get(i);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(plc),
                Some(plp),
                Some(psc),
                Some(psp),
                Some(rsi_val),
                Some(price),
                Some(atr_val),
            ) = (
                p_line_c, p_line_p, p_sig_c, p_sig_p, rsi_c, price_opt, atr_opt,
            ) {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Entry Long: PPO line crosses above PPO signal line AND RSI < rsi_sell_threshold
                if plp <= psp && plc > psc && rsi_val < self.config.rsi_sell_threshold {
                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bullish Crossover: PPO Line {:.2} > Signal {:.2} & RSI {:.2} < {:.2}",
                            plc, psc, rsi_val, self.config.rsi_sell_threshold
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
                            "Bullish Crossover: PPO Line {:.2} > Signal {:.2} & RSI {:.2} < {:.2}",
                            plc, psc, rsi_val, self.config.rsi_sell_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Exit Long or Entry Short
                // Entry Short: PPO line crosses below PPO signal line AND RSI > rsi_buy_threshold
                else if plp >= psp && plc < psc && rsi_val > self.config.rsi_buy_threshold {
                    // Exit any Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bearish Crossover: PPO Line {:.2} < Signal {:.2} & RSI {:.2} > {:.2}",
                            plc, psc, rsi_val, self.config.rsi_buy_threshold
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
                            "Bearish Crossover: PPO Line {:.2} < Signal {:.2} & RSI {:.2} > {:.2}",
                            plc, psc, rsi_val, self.config.rsi_buy_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: PpoRsiTrendConfig = serde_json::from_value(params)?;
        new_config.validate()?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_ppo_rsi_trend_signals() -> Result<()> {
        let config = PpoRsiTrendConfig {
            ppo_fast_period: 2,
            ppo_slow_period: 4,
            ppo_signal_period: 2,
            rsi_period: 2,
            rsi_buy_threshold: 30.0,
            rsi_sell_threshold: 70.0,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = PpoRsiTrend::new(config);

        // Construct data to force a bullish crossover (PPO > Signal) AND RSI < 70
        // Need enough data points.
        let timestamps: Vec<i64> = (0..15).map(|i| i * 1000).collect();
        // Give enough time for indicators to initialize, then create a dip to get RSI down, then jump to get PPO cross.
        let closes = vec![
            10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, // Uptrend
            15.0, 13.0, 10.0, 8.0, // Dip to lower RSI
            12.0, 16.0, 20.0, // Jump to cause PPO cross
        ];
        let highs: Vec<f64> = closes.iter().map(|c| c + 1.0).collect();
        let lows: Vec<f64> = closes.iter().map(|c| c - 1.0).collect();

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "open" => closes.clone(),
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // We should get some signals due to the crossover and momentum change.
        assert!(!signals.is_empty(), "Should generate signals");

        let entry_long = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "buy");
        assert!(entry_long.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_ppo_rsi_trend_short_signals() -> Result<()> {
        let config = PpoRsiTrendConfig {
            ppo_fast_period: 2,
            ppo_slow_period: 4,
            ppo_signal_period: 2,
            rsi_period: 2,
            rsi_buy_threshold: 30.0,
            rsi_sell_threshold: 70.0,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = PpoRsiTrend::new(config);

        // Construct data to force a bearish crossover AND RSI > 30
        let timestamps: Vec<i64> = (0..15).map(|i| i * 1000).collect();
        // Give enough time for indicators to initialize, then spike to get RSI up, then crash to get PPO cross.
        let closes = vec![
            20.0, 19.0, 18.0, 17.0, 16.0, 15.0, 14.0, 13.0, // Downtrend
            15.0, 18.0, 22.0, 25.0, // Spike to raise RSI
            20.0, 15.0, 10.0, // Crash to cause PPO cross
        ];
        let highs: Vec<f64> = closes.iter().map(|c| c + 1.0).collect();
        let lows: Vec<f64> = closes.iter().map(|c| c - 1.0).collect();

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "open" => closes.clone(),
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        assert!(!signals.is_empty(), "Should generate signals");

        let entry_short = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "sell");
        assert!(entry_short.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_ppo_rsi_trend_empty_data() -> Result<()> {
        let config = PpoRsiTrendConfig {
            ppo_fast_period: 2,
            ppo_slow_period: 4,
            ppo_signal_period: 2,
            rsi_period: 2,
            rsi_buy_threshold: 30.0,
            rsi_sell_threshold: 70.0,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = PpoRsiTrend::new(config);

        let df = DataFrame::default();
        let signals = strategy.generate_signals(&df).await?;
        assert!(
            signals.is_empty(),
            "Should return empty vector on empty data"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_ppo_rsi_trend_update_params() -> Result<()> {
        let mut strategy = PpoRsiTrend::new(PpoRsiTrendConfig {
            ppo_fast_period: 12,
            ppo_slow_period: 26,
            ppo_signal_period: 9,
            rsi_period: 14,
            rsi_buy_threshold: 50.0,
            rsi_sell_threshold: 50.0,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "ppo_fast_period": 9,
            "ppo_slow_period": 21,
            "ppo_signal_period": 7,
            "rsi_period": 14,
            "rsi_buy_threshold": 45.0,
            "rsi_sell_threshold": 55.0,
            "atr_period": 20,
            "stop_loss_atr_mult": 3.0,
            "symbol": "NEW_TEST"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.ppo_fast_period, 9);
        assert_eq!(strategy.config.rsi_buy_threshold, 45.0);
        assert_eq!(strategy.config.symbol, "NEW_TEST");

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let invalid_config = PpoRsiTrendConfig {
            ppo_fast_period: 26,
            ppo_slow_period: 12, // fast > slow
            ppo_signal_period: 9,
            rsi_period: 14,
            rsi_buy_threshold: 30.0,
            rsi_sell_threshold: 70.0,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };

        let result = invalid_config.validate();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "ppo_fast_period must be less than ppo_slow_period"
        );

        let invalid_config2 = PpoRsiTrendConfig {
            ppo_fast_period: 12,
            ppo_slow_period: 26,
            ppo_signal_period: 9,
            rsi_period: 14,
            rsi_buy_threshold: 150.0, // > 100
            rsi_sell_threshold: 70.0,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };

        let result2 = invalid_config2.validate();
        assert!(result2.is_err());
        assert_eq!(
            result2.unwrap_err().to_string(),
            "rsi_buy_threshold must be between 0 and 100"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_ppo_rsi_trend_extreme_volatility() -> Result<()> {
        let config = PpoRsiTrendConfig {
            ppo_fast_period: 2,
            ppo_slow_period: 4,
            ppo_signal_period: 2,
            rsi_period: 2,
            rsi_buy_threshold: 30.0,
            rsi_sell_threshold: 70.0,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = PpoRsiTrend::new(config);

        // Construct data with extreme price swings
        let timestamps: Vec<i64> = (0..10).map(|i| i * 1000).collect();
        let closes = vec![
            100.0, 50.0, 200.0, 25.0, 400.0, 50.0, 800.0, 25.0, 1000.0, 10.0,
        ];
        let highs = vec![
            110.0, 60.0, 210.0, 35.0, 410.0, 60.0, 810.0, 35.0, 1010.0, 20.0,
        ];
        let lows = vec![
            90.0, 40.0, 190.0, 15.0, 390.0, 40.0, 790.0, 15.0, 990.0, 5.0,
        ];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "open" => closes.clone(),
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Extreme volatility should still be processed without crashing
        // It should also generate signals
        assert!(
            !signals.is_empty(),
            "Should generate signals under extreme volatility"
        );

        Ok(())
    }
}
