use crate::indicators::{atr, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for SmaCrossoverConfig {}

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
        if data.height() == 0 {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let short_sma_series = sma::calculate(data, self.config.short_period)?;
        let long_sma_series = sma::calculate(data, self.config.long_period)?;

        let short_sma = short_sma_series.f64()?;
        let long_sma = long_sma_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i);

            let s_curr_opt = short_sma.get(i);
            let l_curr_opt = long_sma.get(i);
            let s_prev_opt = short_sma.get(i - 1);
            let l_prev_opt = long_sma.get(i - 1);

            let atr_opt = atr_arr.get(i);

            if let (Some(sc), Some(lc), Some(sp), Some(lp), Some(price), Some(atr_val)) = (
                s_curr_opt, l_curr_opt, s_prev_opt, l_prev_opt, price_opt, atr_opt,
            ) {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Bullish Crossover (Short SMA crosses ABOVE Long SMA)
                if sp <= lp && sc > lc {
                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short (Buy to cover)
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("SMA Crossover Up: Short {:.2} > Long {:.2}", sc, lc),
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
                        reason: format!("SMA Crossover Up: Short {:.2} > Long {:.2}", sc, lc),
                        timestamp_ms: timestamp,
                    });
                }
                // Bearish Crossover (Short SMA crosses BELOW Long SMA)
                else if sp >= lp && sc < lc {
                    // Exit any Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("SMA Crossover Down: Short {:.2} < Long {:.2}", sc, lc),
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
                        reason: format!("SMA Crossover Down: Short {:.2} < Long {:.2}", sc, lc),
                        timestamp_ms: timestamp,
                    });
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

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let config = SmaCrossoverConfig {
            short_period: 2,
            long_period: 3,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };

        let strategy = SmaCrossover::new(config);
        let empty_df = DataFrame::empty();
        let signals = strategy.generate_signals(&empty_df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_sma_crossover_signals() -> Result<()> {
        let config = SmaCrossoverConfig {
            short_period: 2,
            long_period: 3,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = SmaCrossover::new(config);

        // Data engineered to produce crossover
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000],
            "open"  => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "high"  => &[11.0, 12.0, 13.0, 14.0, 15.0, 12.0, 10.0],
            "low"   => &[9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0],
            "close" => &[10.0, 10.0, 10.0, 14.0, 16.0, 10.0, 8.0],
            "volume"=> &[100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0]
        )?;

        // Close: 10, 10, 10, 14, 16, 10, 8
        // SMA(2): -, 10, 10, 12, 15, 13, 9
        // SMA(3): -, -, 10, 11.33, 13.33, 13.33, 11.33

        // i=2: SMA(2)=10, SMA(3)=10
        // i=3: SMA(2)=12, SMA(3)=11.33 (Short > Long -> LONG ENTRY)
        // i=4: SMA(2)=15, SMA(3)=13.33
        // i=5: SMA(2)=13, SMA(3)=13.33 (Short < Long -> SHORT ENTRY)

        let signals = strategy.generate_signals(&df).await?;

        let entries_long: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();
        assert_eq!(entries_long.len(), 1);
        assert_eq!(entries_long[0].timestamp_ms, 4000); // Index 3

        let entries_short: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "sell")
            .collect();
        assert_eq!(entries_short.len(), 1);
        assert_eq!(entries_short[0].timestamp_ms, 6000); // Index 5

        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let mut strategy = SmaCrossover::new(SmaCrossoverConfig {
            short_period: 10,
            long_period: 20,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "short_period": 5,
            "long_period": 15,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.short_period, 5);
        assert_eq!(strategy.config.long_period, 15);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
