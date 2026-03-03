use crate::indicators::{atr, awesome_oscillator};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwesomeOscillatorConfig {
    pub fast_period: usize,
    pub slow_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for AwesomeOscillatorConfig {}

pub struct AwesomeOscillator {
    config: AwesomeOscillatorConfig,
}

impl AwesomeOscillator {
    pub fn new(config: AwesomeOscillatorConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for AwesomeOscillator {
    fn name(&self) -> &str {
        "AwesomeOscillator"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate Awesome Oscillator
        let ao_series =
            awesome_oscillator::calculate(data, self.config.fast_period, self.config.slow_period)?;
        let ao_arr = ao_series.f64()?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let ao_curr = ao_arr.get(i);
            let ao_prev = ao_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (Some(ao_c), Some(ao_p), Some(price), Some(atr_val)) =
                (ao_curr, ao_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Bullish Crossover (AO crosses ABOVE Zero)
                if ao_p <= 0.0 && ao_c > 0.0 {
                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short (Buy to cover)
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("AO Crossover Up: AO {:.2} > 0.0", ao_c),
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
                        reason: format!("AO Crossover Up: AO {:.2} > 0.0", ao_c),
                        timestamp_ms: timestamp,
                    });
                }
                // Bearish Crossover (AO crosses BELOW Zero)
                else if ao_p >= 0.0 && ao_c < 0.0 {
                    // Exit any Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("AO Crossover Down: AO {:.2} < 0.0", ao_c),
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
                        reason: format!("AO Crossover Down: AO {:.2} < 0.0", ao_c),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: AwesomeOscillatorConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_ao_signals() -> Result<()> {
        let config = AwesomeOscillatorConfig {
            fast_period: 2,
            slow_period: 4,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = AwesomeOscillator::new(config);

        let _df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000],
            "open"  => &[10.0, 10.0, 10.0, 15.0, 20.0, 10.0, 5.0],
            "high"  => &[10.0, 10.0, 10.0, 15.0, 20.0, 10.0, 5.0],
            "low"   => &[10.0, 10.0, 10.0, 15.0, 20.0, 10.0, 5.0],
            "close" => &[10.0, 10.0, 10.0, 15.0, 20.0, 10.0, 5.0]
        )?;

        // median: [10.0, 10.0, 10.0, 15.0, 20.0, 10.0, 5.0]
        // SMA(2): -, 10.0, 10.0, 12.5, 17.5, 15.0, 7.5
        // SMA(4): -, -, -, 11.25, 13.75, 13.75, 12.5
        // AO: -, -, -, 1.25, 3.75, 1.25, -5.0

        // Let's print out the AO values if we want to be sure, but we know them.
        // Wait, at i=2, ao_prev is None?
        // i=0: ao=None
        // i=1: ao=None
        // i=2: ao=None
        // i=3: ao_prev=None, ao_curr=1.25. (ao_prev <= 0.0 will fail because it's None)
        // i=4: ao_prev=1.25, ao_curr=3.75
        // i=5: ao_prev=3.75, ao_curr=1.25
        // i=6: ao_prev=1.25, ao_curr=-5.0 (Crosses below 0) -> Short entry expected here
        //
        // So we won't get a Long entry at i=3 because ao_prev is None!
        // We need an initial value <= 0 to trigger the cross. Let's add more bars.

        let df = df!(
            "timestamp_unix_ms" => &[0i64, 1000, 2000, 3000, 4000, 5000, 6000, 7000],
            "open"  => &[10.0, 10.0, 10.0, 10.0, 15.0, 20.0, 10.0, 5.0],
            "high"  => &[10.0, 10.0, 10.0, 10.0, 15.0, 20.0, 10.0, 5.0],
            "low"   => &[10.0, 10.0, 10.0, 10.0, 15.0, 20.0, 10.0, 5.0],
            "close" => &[10.0, 10.0, 10.0, 10.0, 15.0, 20.0, 10.0, 5.0]
        )?;

        // median: [10.0, 10.0, 10.0, 10.0, 15.0, 20.0, 10.0, 5.0]
        // SMA(2): -, 10.0, 10.0, 10.0, 12.5, 17.5, 15.0, 7.5
        // SMA(4): -, -, -, 10.0, 11.25, 13.75, 13.75, 12.5
        // AO: -, -, -, 0.0, 1.25, 3.75, 1.25, -5.0

        // i=3: AO = 0.0
        // i=4: AO = 1.25. ao_prev = 0.0. (0.0 <= 0.0 && 1.25 > 0.0) -> LONG ENTRY at 4000ms

        let signals = strategy.generate_signals(&df).await?;

        // Entry Long at i=4 (4000ms): AO crosses above 0
        let entries_long: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();
        assert_eq!(entries_long.len(), 1);
        assert_eq!(entries_long[0].timestamp_ms, 4000);

        // Entry Short at i=6 (7000ms): AO crosses below 0
        let entries_short: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "sell")
            .collect();
        assert_eq!(entries_short.len(), 1);
        assert_eq!(entries_short[0].timestamp_ms, 7000);

        Ok(())
    }

    #[tokio::test]
    async fn test_ao_update_params() -> Result<()> {
        let mut strategy = AwesomeOscillator::new(AwesomeOscillatorConfig {
            fast_period: 5,
            slow_period: 34,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "fast_period": 10,
            "slow_period": 50,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 20,
            "symbol": "NEW_TEST"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.fast_period, 10);
        assert_eq!(strategy.config.slow_period, 50);
        assert_eq!(strategy.config.symbol, "NEW_TEST");

        Ok(())
    }
}
