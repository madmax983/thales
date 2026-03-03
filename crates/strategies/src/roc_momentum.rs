use crate::indicators::atr;
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RocMomentumConfig {
    pub roc_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for RocMomentumConfig {}

pub struct RocMomentum {
    config: RocMomentumConfig,
}

impl RocMomentum {
    pub fn new(config: RocMomentumConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for RocMomentum {
    fn name(&self) -> &str {
        "RocMomentum"
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

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let one_dec = Decimal::ONE;
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));

        let roc_period = self.config.roc_period;

        if close_arr.len() <= roc_period + 1 {
            return Ok(signals);
        }

        // Iterate through data to calculate ROC and check crossovers
        // We only check from roc_period + 1 to check previous ROC
        for i in (roc_period + 1)..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            // ROC current
            let curr_price = close_arr.get(i);
            let curr_prev = close_arr.get(i - roc_period);

            // ROC previous
            let prev_price = close_arr.get(i - 1);
            let prev_prev = close_arr.get(i - 1 - roc_period);

            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(cp), Some(cp_prev), Some(pp), Some(pp_prev)) = (curr_price, curr_prev, prev_price, prev_prev) {
                if cp_prev == 0.0 || pp_prev == 0.0 {
                    continue;
                }

                let roc_curr = (cp - cp_prev) / cp_prev * 100.0;
                let roc_prev = (pp - pp_prev) / pp_prev * 100.0;

                let price_dec = Decimal::from_f64_retain(cp).unwrap_or(Decimal::ZERO);

                // Bullish Crossover (Entry)
                // ROC crosses ABOVE 0
                if roc_curr > 0.0 && roc_prev <= 0.0 {
                    let sl = if let Some(atr_val) = atr_opt {
                        price_dec - (atr_val * atr_mult_dec)
                    } else {
                        price_dec * (one_dec - Decimal::from_f64_retain(0.05).unwrap())
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
                            "Bullish Crossover: ROC crossed above 0 ({:.2})",
                            roc_curr
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Bearish Crossover (Exit)
                // ROC crosses BELOW 0
                if roc_curr < 0.0 && roc_prev >= 0.0 {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bearish Crossover: ROC crossed below 0 ({:.2})",
                            roc_curr
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: RocMomentumConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_roc_signals() -> Result<()> {
        let config = RocMomentumConfig {
            roc_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = RocMomentum::new(config);

        let closes = vec![100.0, 95.0, 90.0, 85.0, 88.0, 95.0, 105.0, 110.0, 105.0, 100.0];
        let highs = closes.iter().map(|c| c + 1.0).collect::<Vec<_>>();
        let lows = closes.iter().map(|c| c - 1.0).collect::<Vec<_>>();
        let times = (0..10).map(|i| i as i64 * 1000).collect::<Vec<_>>();

        let df = df!(
            "timestamp_unix_ms" => times,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        assert!(!entries.is_empty(), "Expected at least one entry signal");
        assert!(!exits.is_empty(), "Expected at least one exit signal");

        let entry = &entries[0];
        assert!(entry.stop_loss.is_some());

        Ok(())
    }
}
