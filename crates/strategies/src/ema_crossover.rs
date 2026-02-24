use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig};
use crate::indicators::ema;
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaCrossoverConfig {
    pub short_window: usize,
    pub long_window: usize,
    pub stop_loss_pct: f64,
    pub symbol: String,
}

impl StrategyConfig for EmaCrossoverConfig {}

pub struct EmaCrossover {
    config: EmaCrossoverConfig,
}

impl EmaCrossover {
    pub fn new(config: EmaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for EmaCrossover {
    fn name(&self) -> &str {
        "EmaCrossover"
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let short_ema_series = ema::calculate(data, self.config.short_window)?;
        let long_ema_series = ema::calculate(data, self.config.long_window)?;

        let short_ema = short_ema_series.f64()?;
        let long_ema = long_ema_series.f64()?;

        let mut signals = Vec::new();
        let mut entry_price: Option<Decimal> = None;
        let stop_loss_pct_dec = Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));

            // Ensure we have EMA values
            let s_curr_opt = short_ema.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let l_curr_opt = long_ema.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let s_prev_opt = short_ema.get(i - 1).and_then(|v| Decimal::from_f64_retain(v));
            let l_prev_opt = long_ema.get(i - 1).and_then(|v| Decimal::from_f64_retain(v));

            if let (Some(sc), Some(lc), Some(sp), Some(lp), Some(price)) = (s_curr_opt, l_curr_opt, s_prev_opt, l_prev_opt, price_opt) {
                // Check for Exit first (Stop Loss or Reverse Crossover)
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
                        continue; // Processed for this bar
                    }

                    // Bearish Crossover (Exit)
                    if sc < lc && sp >= lp {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            stop_loss: None,
                            take_profit: None,
                            reason: format!("Bearish Crossover: Short {} < Long {}", sc.round_dp(2), lc.round_dp(2)),
                            timestamp_ms: timestamp,
                        });
                        entry_price = None;
                        continue;
                    }
                }

                // Check for Entry
                if entry_price.is_none() {
                    // Bullish Crossover (Entry)
                    if sc > lc && sp <= lp {
                        let sl = price * (one_dec - stop_loss_pct_dec);
                        // TP None for trend following

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: None,
                            reason: format!("Bullish Crossover: Short {} > Long {}", sc.round_dp(2), lc.round_dp(2)),
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
        let new_config: EmaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_ema_crossover_signals() -> Result<()> {
        // Setup: Short window 2, Long window 3.
        // Data designed to produce crossover.
        let config = EmaCrossoverConfig {
            short_window: 2,
            long_window: 3,
            stop_loss_pct: 0.2, // Increased SL to avoid triggering it before crossover
            symbol: "TEST".to_string(),
        };
        let strategy = EmaCrossover::new(config);

        // Prices: 10, 10, 10, 12, 14, 10
        // S_EMA(2):
        // 0: None
        // 1: 10 (seed)
        // 2: 10
        // 3: 12*2/3 + 10*1/3 = 8 + 3.33 = 11.33
        // 4: 14*2/3 + 11.33*1/3 = 9.33 + 3.77 = 13.1
        // 5: 10*2/3 + 13.1*1/3 = 6.66 + 4.36 = 11.0

        // L_EMA(3):
        // 0: None
        // 1: None
        // 2: 10 (seed)
        // 3: 12*0.5 + 10*0.5 = 11
        // 4: 14*0.5 + 11*0.5 = 12.5
        // 5: 10*0.5 + 12.5*0.5 = 11.25

        // Comparison:
        // 2: S=10, L=10. S <= L.
        // 3: S=11.33, L=11. S > L. Crossover! Entry.
        // 4: S=13.1, L=12.5. S > L. Hold.
        // 5: S=11.0, L=11.25. S < L. Crossover! Exit.

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000],
            "close" => &[10.0, 10.0, 10.0, 12.0, 14.0, 10.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect Entry at index 3, Exit at index 5.
        assert_eq!(signals.len(), 2);

        let entry = &signals[0];
        assert_eq!(entry.signal_type, SignalType::Entry);
        assert_eq!(entry.timestamp_ms, 4000); // Index 3
        assert!(entry.stop_loss.is_some());
        assert!(entry.take_profit.is_none());


        let exit = &signals[1];
        assert_eq!(exit.signal_type, SignalType::Exit);
        assert_eq!(exit.timestamp_ms, 6000); // Index 5
        assert!(exit.reason.contains("Bearish Crossover"));

        Ok(())
    }

    #[tokio::test]
    async fn test_stop_loss() -> Result<()> {
        let config = EmaCrossoverConfig {
            short_window: 2,
            long_window: 3,
            stop_loss_pct: 0.1, // 10% SL
            symbol: "TEST".to_string(),
        };
        let strategy = EmaCrossover::new(config);

        // Prices: 10, 10, 10, 12, 10 (SL hit)
        // Entry at index 3 (Price 12).
        // SL Price = 12 * 0.9 = 10.8.
        // Index 4 Price 10 <= 10.8. Exit.

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "close" => &[10.0, 10.0, 10.0, 12.0, 10.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        assert_eq!(signals.len(), 2);
        assert_eq!(signals[1].signal_type, SignalType::Exit);
        assert!(signals[1].reason.contains("Stop Loss"));

        Ok(())
    }
}
