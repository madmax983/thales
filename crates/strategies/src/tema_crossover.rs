use crate::indicators::{atr, tema};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub atr_period: usize,
    pub atr_mult: f64,
    pub symbol: String,
}

impl StrategyConfig for TemaCrossoverConfig {}

pub struct TemaCrossover {
    config: TemaCrossoverConfig,
}

impl TemaCrossover {
    pub fn new(config: TemaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for TemaCrossover {
    fn name(&self) -> &str {
        "TemaCrossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let short_tema_series = tema::calculate(data, self.config.short_period)?;
        let long_tema_series = tema::calculate(data, self.config.long_period)?;

        let short_tema = short_tema_series.f64()?;
        let long_tema = long_tema_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let atr_mult = self.config.atr_mult;

        let mut in_position = false;
        let mut stop_loss = 0.0;

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i);

            let s_curr_opt = short_tema.get(i);
            let l_curr_opt = long_tema.get(i);
            let s_prev_opt = short_tema.get(i - 1);
            let l_prev_opt = long_tema.get(i - 1);

            let atr_opt = atr_arr.get(i);

            if let (Some(sc), Some(lc), Some(sp), Some(lp), Some(price), Some(atr_val)) = (
                s_curr_opt,
                l_curr_opt,
                s_prev_opt,
                l_prev_opt,
                price_opt,
                atr_opt,
            ) {
                // Exit logic
                if in_position {
                    let mut should_exit = false;
                    let mut exit_reason = String::new();

                    if price <= stop_loss {
                        should_exit = true;
                        exit_reason = "Stop Loss Hit".to_string();
                    } else if sc < lc && sp >= lp {
                        should_exit = true;
                        exit_reason = format!("Bearish Crossover: Short TEMA {:.2} < Long TEMA {:.2}", sc, lc);
                    }

                    if should_exit {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason: exit_reason,
                            timestamp_ms: timestamp,
                        });
                        in_position = false;
                    }
                }

                // Entry logic (after potential exit on the same bar)
                if !in_position {
                    if sc > lc && sp <= lp {
                        let sl = price - (atr_val * atr_mult);
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl),
                            take_profit: None,
                            reason: format!("Bullish Crossover: Short TEMA {:.2} > Long TEMA {:.2}", sc, lc),
                            timestamp_ms: timestamp,
                        });
                        in_position = true;
                        stop_loss = sl;
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: TemaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        // Need a long enough series to calculate TEMA for long_period.
        // Let's use short_period=2, long_period=3, atr_period=3.
        // TEMA(3) needs 3*3 - 2 = 7 bars. We need more than 7 bars.
        let mut closes = Vec::new();
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut timestamps = Vec::new();

        let mut price = 100.0;
        for i in 0..30 {
            if i < 15 {
                price -= 1.0; // Downtrend to push short TEMA below long TEMA
            } else if i < 25 {
                price += 2.0; // Uptrend to generate a bullish crossover
            } else {
                price -= 5.0; // Sharp downtrend to hit stop loss or crossover
            }

            closes.push(price);
            highs.push(price + 2.0);
            lows.push(price - 2.0);
            timestamps.push(i as i64 * 60000);
        }

        df!(
            "timestamp_unix_ms" => timestamps,
            "open" => closes.clone(),
            "high" => highs,
            "low" => lows,
            "close" => closes,
            "volume" => vec![1000.0; 30]
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_tema_crossover_empty_data() -> Result<()> {
        let config = TemaCrossoverConfig {
            short_period: 2,
            long_period: 3,
            atr_period: 3,
            atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };

        let strategy = TemaCrossover::new(config);
        let df = DataFrame::empty();
        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_tema_crossover_signals() -> Result<()> {
        let config = TemaCrossoverConfig {
            short_period: 2,
            long_period: 3,
            atr_period: 3,
            atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };

        let strategy = TemaCrossover::new(config);
        let df = create_test_data();
        let signals = strategy.generate_signals(&df).await?;

        // We expect at least one entry and one exit
        assert!(!signals.is_empty(), "Should generate signals");

        let entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry.is_some(), "Should have an entry signal");
        if let Some(e) = entry {
            assert_eq!(e.side, "buy");
            assert!(e.stop_loss.is_some());
        }

        let exit = signals.iter().find(|s| s.signal_type == SignalType::Exit);
        assert!(exit.is_some(), "Should have an exit signal");
        if let Some(ex) = exit {
            assert_eq!(ex.side, "sell");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_tema_crossover_update_params() -> Result<()> {
        let mut strategy = TemaCrossover::new(TemaCrossoverConfig {
            short_period: 2,
            long_period: 3,
            atr_period: 3,
            atr_mult: 2.0,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "short_period": 5,
            "long_period": 10,
            "atr_period": 14,
            "atr_mult": 1.5,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.short_period, 5);
        assert_eq!(strategy.config.long_period, 10);
        assert_eq!(strategy.config.atr_period, 14);
        assert_eq!(strategy.config.atr_mult, 1.5);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
