use crate::indicators::macd;
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacdConfig {
    pub fast_period: usize,
    pub slow_period: usize,
    pub signal_period: usize,
    pub stop_loss_pct: f64,
    pub symbol: String,
}

impl StrategyConfig for MacdConfig {}

pub struct Macd {
    config: MacdConfig,
}

impl Macd {
    pub fn new(config: MacdConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for Macd {
    fn name(&self) -> &str {
        "Macd"
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

        // Calculate MACD
        let (macd_series, signal_series, _) = macd::calculate(
            data,
            self.config.fast_period,
            self.config.slow_period,
            self.config.signal_period,
        )?;

        let macd_arr = macd_series.f64()?;
        let signal_arr = signal_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_pct_dec =
            Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));

            let m_curr_opt = macd_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let s_curr_opt = signal_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let m_prev_opt = macd_arr
                .get(i - 1)
                .and_then(|v| Decimal::from_f64_retain(v));
            let s_prev_opt = signal_arr
                .get(i - 1)
                .and_then(|v| Decimal::from_f64_retain(v));

            if let (Some(mc), Some(sc), Some(mp), Some(sp), Some(price)) =
                (m_curr_opt, s_curr_opt, m_prev_opt, s_prev_opt, price_opt)
            {
                // Bearish Crossover (Exit) - Stateless
                // MACD crosses BELOW Signal
                if mc < sc && mp >= sp {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bearish Crossover: MACD {} < Signal {}",
                            mc.round_dp(2),
                            sc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Bullish Crossover (Entry) - Stateless
                // MACD crosses ABOVE Signal
                if mc > sc && mp <= sp {
                    let sl = price * (one_dec - stop_loss_pct_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None, // Let trend run
                        reason: format!(
                            "Bullish Crossover: MACD {} > Signal {}",
                            mc.round_dp(2),
                            sc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: MacdConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_macd_stateless_signals() -> Result<()> {
        let config = MacdConfig {
            fast_period: 12,
            slow_period: 26,
            signal_period: 9,
            stop_loss_pct: 0.1,
            symbol: "TEST".to_string(),
        };
        let strategy = Macd::new(config);

        // Sine wave to force crossovers
        let mut closes = Vec::new();
        let mut times = Vec::new();
        for i in 0..100 {
            let val = 100.0 + (i as f64 * 0.2).sin() * 10.0;
            closes.push(val);
            times.push(i as i64 * 1000);
        }

        let df = df!(
            "timestamp_unix_ms" => times,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Should have independent Entry and Exit signals
        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        assert!(!entries.is_empty());
        assert!(!exits.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_stateless_crossover_exit_only() -> Result<()> {
        // Create data that STARTS with MACD > Signal, then crosses down immediately.
        // No Entry signal should be generated, only Exit.

        let config = MacdConfig {
            fast_period: 2,
            slow_period: 5,
            signal_period: 2,
            stop_loss_pct: 0.1,
            symbol: "TEST".to_string(),
        };
        let strategy = Macd::new(config);

        // Need enough points for Slow EMA (5) and Signal EMA (2)
        // Let's generate a sequence where MACD is positive for a while, then crosses down.
        // Rise for 15 periods, then drop.

        let mut closes = Vec::new();
        let mut times = Vec::new();
        // Rise
        for i in 0..15 {
            closes.push(10.0 + i as f64);
            times.push(i as i64 * 1000);
        }
        // Drop sharp
        for i in 15..25 {
            closes.push(25.0 - (i - 15) as f64 * 2.0);
            times.push(i as i64 * 1000);
        }

        let df = df!(
            "timestamp_unix_ms" => times,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        assert!(
            !exits.is_empty(),
            "Should generate exit signal on bearish crossover"
        );

        Ok(())
    }
}
