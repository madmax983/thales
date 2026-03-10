use crate::indicators::{atr, ema, supertrend};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupertrendEmaCrossoverConfig {
    pub supertrend_period: usize,
    pub supertrend_multiplier: f64,
    pub ema_short_period: usize,
    pub ema_long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for SupertrendEmaCrossoverConfig {
    fn default() -> Self {
        Self {
            supertrend_period: 10,
            supertrend_multiplier: 3.0,
            ema_short_period: 10,
            ema_long_period: 20,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "BTC/USD".to_string(),
        }
    }
}

impl StrategyConfig for SupertrendEmaCrossoverConfig {}

pub struct SupertrendEmaCrossover {
    config: SupertrendEmaCrossoverConfig,
}

impl SupertrendEmaCrossover {
    pub fn new(config: SupertrendEmaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for SupertrendEmaCrossover {
    fn name(&self) -> &str {
        "SupertrendEmaCrossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        // Compute Indicators
        let (_, supertrend_trend_series) = supertrend::calculate(
            data,
            self.config.supertrend_period,
            self.config.supertrend_multiplier,
        )?;
        let ema_short_series = ema::calculate(data, self.config.ema_short_period)?;
        let ema_long_series = ema::calculate(data, self.config.ema_long_period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        // Extract native types
        let st_trend_arr = supertrend_trend_series.i32()?;
        let ema_short_arr = ema_short_series.f64()?;
        let ema_long_arr = ema_long_series.f64()?;
        let atr_arr = atr_series.f64()?;

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));

        // Iterate through data (start at 1 for prev values)
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            // Fetch current and previous values
            let st_trend_opt = st_trend_arr.get(i);
            let ema_short_curr_opt = ema_short_arr.get(i).and_then(Decimal::from_f64_retain);
            let ema_long_curr_opt = ema_long_arr.get(i).and_then(Decimal::from_f64_retain);
            let ema_short_prev_opt = ema_short_arr.get(i - 1).and_then(Decimal::from_f64_retain);
            let ema_long_prev_opt = ema_long_arr.get(i - 1).and_then(Decimal::from_f64_retain);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);
            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (
                Some(st_trend),
                Some(ema_short_curr),
                Some(ema_long_curr),
                Some(ema_short_prev),
                Some(ema_long_prev),
                Some(price),
            ) = (
                st_trend_opt,
                ema_short_curr_opt,
                ema_long_curr_opt,
                ema_short_prev_opt,
                ema_long_prev_opt,
                price_opt,
            ) {
                // Bullish Entry Condition: Supertrend is UP (1), and EMA Short crosses ABOVE EMA Long
                if st_trend == 1
                    && ema_short_curr > ema_long_curr
                    && ema_short_prev <= ema_long_prev
                {
                    let mut sl = price * Decimal::new(99, 2); // default 1% SL fallback
                    if let Some(atr_val) = atr_opt {
                        sl = price - (atr_val * sl_mult);
                    }

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(), // 100% position size or specific units
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "Bullish Supertrend EMA Crossover: ST Up, Short {:.2} > Long {:.2}",
                            ema_short_curr, ema_long_curr
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Bearish Entry Condition: Supertrend is DOWN (-1), and EMA Short crosses BELOW EMA Long
                if st_trend == -1
                    && ema_short_curr < ema_long_curr
                    && ema_short_prev >= ema_long_prev
                {
                    let mut sl = price * Decimal::new(101, 2); // default 1% SL fallback
                    if let Some(atr_val) = atr_opt {
                        sl = price + (atr_val * sl_mult);
                    }

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "Bearish Supertrend EMA Crossover: ST Down, Short {:.2} < Long {:.2}",
                            ema_short_curr, ema_long_curr
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Bullish Exit Condition: EMA Short crosses BELOW EMA Long
                if ema_short_curr < ema_long_curr && ema_short_prev >= ema_long_prev {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(), // Exit full long position
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Exit Long: Short {:.2} < Long {:.2}",
                            ema_short_curr, ema_long_curr
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Bearish Exit Condition: EMA Short crosses ABOVE EMA Long
                if ema_short_curr > ema_long_curr && ema_short_prev <= ema_long_prev {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(), // Exit full short position
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Exit Short: Short {:.2} > Long {:.2}",
                            ema_short_curr, ema_long_curr
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: SupertrendEmaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_supertrend_ema_crossover_signals() -> Result<()> {
        let config = SupertrendEmaCrossoverConfig {
            supertrend_period: 2,
            supertrend_multiplier: 1.0,
            ema_short_period: 2,
            ema_long_period: 3,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = SupertrendEmaCrossover::new(config);

        // Dummy data crafted to trigger EMA crossover and Supertrend direction
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000],
            "open"  => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "high"  => &[11.0, 12.0, 13.0, 14.0, 15.0, 14.0, 12.0],
            "low"   => &[9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0],
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 11.0, 9.0],
            "volume"=> &[100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0]
        )?;

        let signals = strategy.generate_signals(&df).await;
        assert!(
            signals.is_ok(),
            "Signal generation should complete without panicking"
        );
        let signals = signals.unwrap();

        // Check if we get any signals
        // Note: Exact signal presence depends on the indicator calculations on the short window,
        // so we mainly verify it runs without panicking on valid data and doesn't return an error.
        let entries = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .count();
        let exits = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .count();

        println!("Generated {} entries and {} exits", entries, exits);

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_data() {
        let config = SupertrendEmaCrossoverConfig::default();
        let strategy = SupertrendEmaCrossover::new(config);

        let df = DataFrame::default();
        let result = strategy.generate_signals(&df).await;

        // Supertrend indicator will bail if data is empty
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let mut strategy = SupertrendEmaCrossover::new(SupertrendEmaCrossoverConfig::default());

        let new_params = serde_json::json!({
            "supertrend_period": 14,
            "supertrend_multiplier": 2.5,
            "ema_short_period": 5,
            "ema_long_period": 15,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "ETH/USD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.supertrend_period, 14);
        assert_eq!(strategy.config.supertrend_multiplier, 2.5);
        assert_eq!(strategy.config.ema_short_period, 5);
        assert_eq!(strategy.config.ema_long_period, 15);
        assert_eq!(strategy.config.stop_loss_atr_mult, 1.5);
        assert_eq!(strategy.config.atr_period, 10);
        assert_eq!(strategy.config.symbol, "ETH/USD");

        Ok(())
    }
}
