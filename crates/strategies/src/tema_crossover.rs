use crate::indicators::{atr, tema};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
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
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0)); // Default 2.0 if missing

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);

            // Ensure we have TEMA values
            let s_curr_opt = short_tema.get(i).and_then(Decimal::from_f64_retain);
            let l_curr_opt = long_tema.get(i).and_then(Decimal::from_f64_retain);
            let s_prev_opt = short_tema.get(i - 1).and_then(Decimal::from_f64_retain);
            let l_prev_opt = long_tema.get(i - 1).and_then(Decimal::from_f64_retain);

            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

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
                        price * Decimal::from_f64_retain(0.95).unwrap_or(Decimal::ZERO)
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
        let new_config: TemaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_tema_crossover_signals() -> Result<()> {
        let config = TemaCrossoverConfig {
            short_period: 2,
            long_period: 4,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = TemaCrossover::new(config);

        // Prices for some TEMA crossover
        // Need a longer warmup period (e.g. 15-20 bars) since TEMA uses 3 EMAs.
        // For EMA length 4, one EMA needs ~4 bars to seed, EMA2 needs 4 more, EMA3 needs 4 more (total ~12 bars).
        let mut closes = vec![10.0; 15];
        closes.extend_from_slice(&[15.0, 20.0, 25.0, 30.0, 35.0, 10.0, 10.0, 10.0, 10.0]);

        let len = closes.len();
        let highs: Vec<f64> = closes.iter().map(|c| c + 0.5).collect();
        let lows: Vec<f64> = closes.iter().map(|c| c - 0.5).collect();
        let timestamps: Vec<i64> = (0..len).map(|i| (i as i64) * 1000).collect();

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect Entry and Exit signals
        assert!(!signals.is_empty());

        let entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry.is_some());

        let sl = entry.unwrap().stop_loss.unwrap();
        assert!(sl > 0.0);

        Ok(())
    }
}