use crate::indicators::{atr, ema, rsi};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaRsiTrendFollowingConfig {
    pub short_ema_period: usize,
    pub long_ema_period: usize,
    pub rsi_period: usize,
    pub rsi_buy_threshold: f64,
    pub rsi_sell_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for EmaRsiTrendFollowingConfig {}

pub struct EmaRsiTrendFollowing {
    config: EmaRsiTrendFollowingConfig,
}

impl EmaRsiTrendFollowing {
    pub fn new(config: EmaRsiTrendFollowingConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for EmaRsiTrendFollowing {
    fn name(&self) -> &str {
        "EmaRsiTrendFollowing"
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

        let short_ema_series = ema::calculate(data, self.config.short_ema_period)?;
        let short_ema_arr = short_ema_series.f64()?;

        let long_ema_series = ema::calculate(data, self.config.long_ema_period)?;
        let long_ema_arr = long_ema_series.f64()?;

        let rsi_series = rsi::calculate(data, self.config.rsi_period)?;
        let rsi_arr = rsi_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let short_c = short_ema_arr.get(i);
            let short_p = short_ema_arr.get(i - 1);
            let long_c = long_ema_arr.get(i);
            let long_p = long_ema_arr.get(i - 1);
            let rsi_c = rsi_arr.get(i);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(sc),
                Some(sp),
                Some(lc),
                Some(lp),
                Some(rsi_val),
                Some(price),
                Some(atr_val),
            ) = (short_c, short_p, long_c, long_p, rsi_c, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Entry Condition: Short EMA crosses ABOVE Long EMA and RSI > Buy Threshold
                if sp <= lp && sc > lc && rsi_val > self.config.rsi_buy_threshold {
                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bullish Crossover: Short {:.2} > Long {:.2} & RSI {:.2} > {:.2}",
                            sc, lc, rsi_val, self.config.rsi_buy_threshold
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
                            "Bullish Crossover: Short {:.2} > Long {:.2} & RSI {:.2} > {:.2}",
                            sc, lc, rsi_val, self.config.rsi_buy_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Short Entry Condition: Short EMA crosses BELOW Long EMA and RSI < Sell Threshold
                else if sp >= lp && sc < lc && rsi_val < self.config.rsi_sell_threshold {
                    // Exit any Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bearish Crossover: Short {:.2} < Long {:.2} & RSI {:.2} < {:.2}",
                            sc, lc, rsi_val, self.config.rsi_sell_threshold
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
                        side: "sell".to_string(), // Entry Short
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "Bearish Crossover: Short {:.2} < Long {:.2} & RSI {:.2} < {:.2}",
                            sc, lc, rsi_val, self.config.rsi_sell_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: EmaRsiTrendFollowingConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_ema_rsi_trend_signals() -> Result<()> {
        let config = EmaRsiTrendFollowingConfig {
            short_ema_period: 2,
            long_ema_period: 3,
            rsi_period: 2,
            rsi_buy_threshold: 50.0,
            rsi_sell_threshold: 50.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = EmaRsiTrendFollowing::new(config);

        let timestamps = vec![1000i64, 2000, 3000, 4000, 5000, 6000];
        // Prices constructed to force an EMA cross AND a specific RSI direction
        // To get RSI > 50, price needs to go up strongly.
        let closes = vec![10.0, 10.0, 10.0, 15.0, 15.0, 10.0];
        let highs = vec![10.0, 10.0, 10.0, 16.0, 15.0, 11.0];
        let lows = vec![10.0, 10.0, 10.0, 10.0, 14.0, 9.0];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "open" => closes.clone(),
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect Entry at index 3 (4000) or 4 (5000) depending on exact EMA lag
        assert!(!signals.is_empty(), "Should generate signals");

        let entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_ema_rsi_trend_short_signals() -> Result<()> {
        let config = EmaRsiTrendFollowingConfig {
            short_ema_period: 2,
            long_ema_period: 3,
            rsi_period: 2,
            rsi_buy_threshold: 50.0,
            rsi_sell_threshold: 50.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = EmaRsiTrendFollowing::new(config);

        let timestamps = vec![1000i64, 2000, 3000, 4000, 5000, 6000];
        // Prices constructed to force a bearish EMA cross AND a specific RSI direction
        // To get RSI < 50, price needs to go down strongly.
        let closes = vec![20.0, 20.0, 20.0, 10.0, 10.0, 15.0];
        let highs = vec![20.0, 20.0, 20.0, 11.0, 11.0, 16.0];
        let lows = vec![20.0, 20.0, 20.0, 9.0, 9.0, 14.0];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "open" => closes.clone(),
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect Entry at index 3 (4000) or 4 (5000) depending on exact EMA lag
        assert!(!signals.is_empty(), "Should generate signals");

        let entry = signals.iter().find(|s| s.signal_type == SignalType::Entry && s.side == "sell");
        assert!(entry.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_ema_rsi_trend_empty_data() -> Result<()> {
        let config = EmaRsiTrendFollowingConfig {
            short_ema_period: 2,
            long_ema_period: 3,
            rsi_period: 2,
            rsi_buy_threshold: 50.0,
            rsi_sell_threshold: 50.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = EmaRsiTrendFollowing::new(config);

        let df = DataFrame::default();
        let signals = strategy.generate_signals(&df).await;
        assert!(signals.is_err(), "Should return error on empty data");

        Ok(())
    }

    #[tokio::test]
    async fn test_ema_rsi_trend_update_params() -> Result<()> {
        let mut strategy = EmaRsiTrendFollowing::new(EmaRsiTrendFollowingConfig {
            short_ema_period: 5,
            long_ema_period: 20,
            rsi_period: 14,
            rsi_buy_threshold: 50.0,
            rsi_sell_threshold: 50.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "short_ema_period": 9,
            "long_ema_period": 21,
            "rsi_period": 14,
            "rsi_buy_threshold": 55.0,
            "rsi_sell_threshold": 45.0,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 20,
            "symbol": "NEW_TEST"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.short_ema_period, 9);
        assert_eq!(strategy.config.rsi_buy_threshold, 55.0);
        assert_eq!(strategy.config.symbol, "NEW_TEST");

        Ok(())
    }
}
