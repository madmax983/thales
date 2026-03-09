use crate::indicators::{atr, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmaCrossoverConfig {
    pub short_window: usize,
    pub long_window: usize,
    pub stop_loss_pct: f64,
    pub atr_period: usize,
    pub atr_mult: f64,
    pub symbol: String,
}

impl StrategyConfig for SmaCrossoverConfig {}

impl Default for SmaCrossoverConfig {
    fn default() -> Self {
        Self {
            short_window: 9,
            long_window: 21,
            stop_loss_pct: 0.05,
            atr_period: 14,
            atr_mult: 2.0,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

pub struct SmaCrossover {
    config: SmaCrossoverConfig,
}

impl SmaCrossover {
    pub fn new(config: SmaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for SmaCrossover {
    fn name(&self) -> &str {
        "SmaCrossover"
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

        let short_sma_series = sma::calculate(data, self.config.short_window)?;
        let short_sma_arr = short_sma_series.f64()?;

        let long_sma_series = sma::calculate(data, self.config.long_window)?;
        let long_sma_arr = long_sma_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_pct_dec =
            Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.atr_mult).unwrap_or(Decimal::new(2, 0));

        let mut in_position = false;
        let mut entry_price = Decimal::ZERO;

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);
            let short_sma_curr = short_sma_arr.get(i).and_then(Decimal::from_f64_retain);
            let long_sma_curr = long_sma_arr.get(i).and_then(Decimal::from_f64_retain);
            let short_sma_prev = short_sma_arr.get(i - 1).and_then(Decimal::from_f64_retain);
            let long_sma_prev = long_sma_arr.get(i - 1).and_then(Decimal::from_f64_retain);
            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (
                Some(price),
                Some(short_curr),
                Some(long_curr),
                Some(short_prev),
                Some(long_prev),
            ) = (
                price_opt,
                short_sma_curr,
                long_sma_curr,
                short_sma_prev,
                long_sma_prev,
            ) {
                if in_position {
                    // Check for exit
                    let stop_loss_price = if let Some(atr_val) = atr_opt {
                        entry_price - (atr_val * atr_mult_dec)
                    } else {
                        entry_price * (one_dec - stop_loss_pct_dec)
                    };

                    if price <= stop_loss_price {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason: "Stop Loss Hit".to_string(),
                            timestamp_ms: timestamp,
                        });
                        in_position = false;
                        continue;
                    }

                    if short_curr < long_curr {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            stop_loss: None,
                            take_profit: None,
                            reason: "SMA Crossover Down".to_string(),
                            timestamp_ms: timestamp,
                        });
                        in_position = false;
                    }
                } else {
                    // Check for entry
                    if short_curr > long_curr && short_prev <= long_prev {
                        let sl = if let Some(atr_val) = atr_opt {
                            price - (atr_val * atr_mult_dec)
                        } else {
                            price * (one_dec - stop_loss_pct_dec)
                        };

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: None,
                            reason: "SMA Crossover Up".to_string(),
                            timestamp_ms: timestamp,
                        });
                        in_position = true;
                        entry_price = price;
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: SmaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        // Need enough data points for the 21 period long window and some trend.
        // We will mock price such that short crosses over long
        let len = 40;
        let mut closes = Vec::with_capacity(len);
        let mut highs = Vec::with_capacity(len);
        let mut lows = Vec::with_capacity(len);
        let mut timestamps = Vec::with_capacity(len);

        // First 25 periods: flat or slightly downtrending
        for i in 0..25 {
            let price = 100.0 - (i as f64) * 0.1;
            closes.push(price);
            highs.push(price + 1.0);
            lows.push(price - 1.0);
            timestamps.push((i * 1000) as i64);
        }

        // Period 25-30: sharp uptrend causing short SMA to cross above long SMA
        for i in 25..30 {
            let price = 100.0 + (i as f64) * 2.0;
            closes.push(price);
            highs.push(price + 2.0);
            lows.push(price - 2.0);
            timestamps.push((i * 1000) as i64);
        }

        // Period 30-40: sharp downtrend causing short SMA to cross below long SMA
        for i in 30..40 {
            let price = 150.0 - (i as f64) * 3.0;
            closes.push(price);
            highs.push(price + 2.0);
            lows.push(price - 2.0);
            timestamps.push((i * 1000) as i64);
        }

        df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_sma_crossover() -> Result<()> {
        let config = SmaCrossoverConfig {
            short_window: 5,
            long_window: 10,
            stop_loss_pct: 0.1,
            atr_period: 5,
            atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = SmaCrossover::new(config);

        let df = create_test_data();

        let signals = strategy.generate_signals(&df).await?;

        // We should see an entry and then an exit.
        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        assert!(
            !entries.is_empty(),
            "Should generate at least one entry signal"
        );
        assert!(
            !exits.is_empty(),
            "Should generate at least one exit signal"
        );

        let first_entry = entries[0];
        assert_eq!(first_entry.side, "buy");
        assert!(first_entry.reason.contains("SMA Crossover Up"));
        assert!(first_entry.stop_loss.is_some());

        let first_exit = exits[0];
        assert_eq!(first_exit.side, "sell");
        assert!(
            first_exit.reason.contains("SMA Crossover Down")
                || first_exit.reason.contains("Stop Loss Hit")
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = SmaCrossover::new(SmaCrossoverConfig::default());

        let new_params = serde_json::json!({
            "short_window": 10,
            "long_window": 30,
            "stop_loss_pct": 0.08,
            "atr_period": 14,
            "atr_mult": 1.5,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.short_window, 10);
        assert_eq!(strategy.config.long_window, 30);
        assert_eq!(strategy.config.stop_loss_pct, 0.08);

        Ok(())
    }
}
