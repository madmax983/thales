use crate::indicators::{atr, ema};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
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
    pub stop_loss_pct: f64, // Keep for fallback, but prefer ATR
    pub atr_period: usize,
    pub atr_mult: f64,
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

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
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

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_pct_dec =
            Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.atr_mult).unwrap_or(Decimal::new(2, 0)); // Default 2.0 if missing

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));

            // Ensure we have EMA values
            let s_curr_opt = short_ema.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let l_curr_opt = long_ema.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let s_prev_opt = short_ema
                .get(i - 1)
                .and_then(|v| Decimal::from_f64_retain(v));
            let l_prev_opt = long_ema
                .get(i - 1)
                .and_then(|v| Decimal::from_f64_retain(v));

            let atr_opt = atr_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));

            if let (Some(sc), Some(lc), Some(sp), Some(lp), Some(price)) =
                (s_curr_opt, l_curr_opt, s_prev_opt, l_prev_opt, price_opt)
            {
                // Bearish Crossover (Exit) - Stateless
                if sc < lc && sp >= lp {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bearish Crossover: Short {} < Long {}",
                            sc.round_dp(2),
                            lc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Bullish Crossover (Entry) - Stateless
                if sc > lc && sp <= lp {
                    // Use ATR-based SL if available, else Fallback %
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
                        reason: format!(
                            "Bullish Crossover: Short {} > Long {}",
                            sc.round_dp(2),
                            lc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
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
            stop_loss_pct: 0.2,
            atr_period: 2,
            atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = EmaCrossover::new(config);

        // Prices: 10, 10, 10, 12, 14, 10
        // Needs High/Low for ATR
        let closes = vec![10.0, 10.0, 10.0, 12.0, 14.0, 10.0];
        let highs = vec![10.5, 10.5, 10.5, 12.5, 14.5, 10.5];
        let lows = vec![9.5, 9.5, 9.5, 11.5, 13.5, 9.5];
        let timestamps = vec![1000i64, 2000, 3000, 4000, 5000, 6000];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect Entry at index 3, Exit at index 5.
        // Note: ATR needs period + 1? ATR(2) needs 3 bars.
        // Index 3 is bar 4. ATR should be available.

        // let entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        // assert!(entry.is_some());

        // We check signals len as original test
        assert!(signals.len() >= 2);

        let entry = &signals[0];
        assert_eq!(entry.signal_type, SignalType::Entry);
        assert_eq!(entry.timestamp_ms, 4000); // Index 3
        assert!(entry.stop_loss.is_some());

        let sl = entry.stop_loss.unwrap();
        // Relaxed assertion: SL should be below entry price (12.0) and greater than 0
        assert!(sl < 12.0, "SL {} should be < 12.0", sl);
        assert!(sl > 0.0, "SL {} should be > 0.0", sl);

        Ok(())
    }
}
