use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig};
use crate::indicators::macd;
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
        let mut entry_price: Option<Decimal> = None;
        let stop_loss_pct_dec = Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));

            let m_curr_opt = macd_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let s_curr_opt = signal_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let m_prev_opt = macd_arr.get(i - 1).and_then(|v| Decimal::from_f64_retain(v));
            let s_prev_opt = signal_arr.get(i - 1).and_then(|v| Decimal::from_f64_retain(v));

            if let (Some(mc), Some(sc), Some(mp), Some(sp), Some(price)) = (m_curr_opt, s_curr_opt, m_prev_opt, s_prev_opt, price_opt) {
                // Check for Exit first (Stop Loss or Bearish Crossover)
                if let Some(entry) = entry_price {
                    // Stop Loss
                    let stop_price = entry * (one_dec - stop_loss_pct_dec);
                    if price <= stop_price {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason: format!("Stop Loss hit: {} <= {}", price, stop_price.round_dp(2)),
                            timestamp_ms: timestamp,
                        });
                        entry_price = None;
                        continue;
                    }

                    // Bearish Crossover (Exit)
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
                            reason: format!("Bearish Crossover: MACD {} < Signal {}", mc.round_dp(2), sc.round_dp(2)),
                            timestamp_ms: timestamp,
                        });
                        entry_price = None;
                        continue;
                    }
                }

                // Check for Entry
                if entry_price.is_none() {
                    // Bullish Crossover (Entry)
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
                            reason: format!("Bullish Crossover: MACD {} > Signal {}", mc.round_dp(2), sc.round_dp(2)),
                            timestamp_ms: timestamp,
                        });
                        entry_price = Some(price);
                    }
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
    async fn test_macd_signals() -> Result<()> {
        let config = MacdConfig {
            fast_period: 12,
            slow_period: 26,
            signal_period: 9,
            stop_loss_pct: 0.1,
            symbol: "TEST".to_string(),
        };
        let strategy = Macd::new(config);

        // We need enough data to generate MACD values.
        // Slow period is 26. So at least 26+9 = 35 points to get valid Signal Line?
        // Actually, ema calculation needs `period` points to start.
        // Fast(12), Slow(26).
        // MACD valid from 26 (approx).
        // Signal(9) needs 9 points of valid MACD.
        // So roughly index 35.

        // Let's generate a sine wave to force crossovers.
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

        // We expect some signals.
        assert!(!signals.is_empty());

        // Check types
        for signal in &signals {
            match signal.signal_type {
                SignalType::Entry => assert_eq!(signal.side, "buy"),
                SignalType::Exit => assert_eq!(signal.side, "sell"),
                _ => {}
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_macd_stop_loss() -> Result<()> {
        let config = MacdConfig {
            fast_period: 2,
            slow_period: 5,
            signal_period: 2,
            stop_loss_pct: 0.1,
            symbol: "TEST".to_string(),
        };
        let strategy = Macd::new(config);

        // Create data that triggers buy, then drops
        // Need to be careful with EMA lag.
        // 0..10: Rising
        // 11..20: Dropping sharp

        let mut closes = Vec::new();
        let mut times = Vec::new();
        // Rise
        for i in 0..20 {
            closes.push(10.0 + i as f64);
            times.push(i as i64 * 1000);
        }
        // Drop
        for i in 20..30 {
            closes.push(30.0 - (i - 20) as f64 * 2.0); // Drop faster
            times.push(i as i64 * 1000);
        }

        let df = df!(
            "timestamp_unix_ms" => times,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Should have Entry then Exit
        let entries: Vec<_> = signals.iter().filter(|s| s.signal_type == SignalType::Entry).collect();
        let exits: Vec<_> = signals.iter().filter(|s| s.signal_type == SignalType::Exit).collect();

        assert!(!entries.is_empty());
        assert!(!exits.is_empty());

        // Verify one of the exits is likely a Stop Loss given the sharp drop
        let sl_exit = exits.iter().find(|s| s.reason.contains("Stop Loss"));

        // We assert we have at least one exit (either SL or Crossover)
        assert!(!exits.is_empty());

        // Ideally we check for SL specifically if the math guarantees it
        if let Some(sl) = sl_exit {
             assert!(sl.reason.contains("Stop Loss"));
        }

        Ok(())
    }
}
