//! The CCI Momentum Strategy
//!
//! Uses the Commodity Channel Index to identify momentum and overbought/oversold conditions.
//!
use crate::indicators::{atr, cci};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CciMomentumConfig {
    pub period: usize,
    pub buy_threshold: f64,
    pub sell_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for CciMomentumConfig {}

pub struct CciMomentum {
    config: CciMomentumConfig,
}

impl CciMomentum {
    pub fn new(config: CciMomentumConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for CciMomentum {
    fn name(&self) -> &str {
        "CciMomentum"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.period + 1 {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?.f64()?;
        let time_series = data.column("timestamp_unix_ms")?;
        // Handle potentially different int types for timestamp
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate CCI
        let cci_series = cci::calculate(data, self.config.period)?;
        let cci_arr = cci_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);

        for i in self.config.period..data.height() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_series.get(i);

            let cci_curr = cci_arr.get(i);
            let cci_prev = cci_arr.get(i - 1);

            let atr_val = atr_arr.get(i);

            if let (Some(price), Some(curr), Some(prev)) = (price_opt, cci_curr, cci_prev) {
                // Buy Signal: Crossover Buy Threshold (e.g. 100)
                if curr > self.config.buy_threshold && prev <= self.config.buy_threshold {
                    let sl = if let Some(atr) = atr_val {
                        let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                        let atr_dec = Decimal::from_f64_retain(atr).unwrap_or(Decimal::ZERO);
                        Some(
                            (price_dec - (atr_dec * stop_loss_mult))
                                .to_f64()
                                .unwrap_or(0.0),
                        )
                    } else {
                        None
                    };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: sl,
                        take_profit: None,
                        reason: format!(
                            "CCI Momentum Buy: CCI {:.2} crossed above {:.2}",
                            curr, self.config.buy_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Sell Signal (Exit): Crossunder Sell Threshold (e.g. 0)
                if curr < self.config.sell_threshold && prev >= self.config.sell_threshold {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "CCI Momentum Sell: CCI {:.2} crossed below {:.2}",
                            curr, self.config.sell_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: CciMomentumConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_cci_momentum_long_entry() -> Result<()> {
        let config = CciMomentumConfig {
            period: 3,
            buy_threshold: 100.0,
            sell_threshold: 0.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 3,
            symbol: "TEST".to_string(),
        };
        let strategy = CciMomentum::new(config);

        // Construct data where CCI crosses 100.
        // We make it 21.0 to be sure > 100.

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "high" => &[10.0, 10.0, 10.0, 21.0],
            "low" => &[10.0, 10.0, 10.0, 21.0],
            "close" => &[10.0, 10.0, 10.0, 21.0]
        )?;

        // CCI[2] is 0.
        // CCI[3] should be > 100.

        let signals = strategy.generate_signals(&df).await?;

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp_ms, 4000);
        assert!(entries[0].reason.contains("CCI Momentum Buy"));

        Ok(())
    }

    #[tokio::test]
    async fn test_cci_momentum_long_exit() -> Result<()> {
        let config = CciMomentumConfig {
            period: 3,
            buy_threshold: 100.0,
            sell_threshold: 0.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 3,
            symbol: "TEST".to_string(),
        };
        let strategy = CciMomentum::new(config);

        // We need CCI to go from >= 0 to < 0.

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "high" => &[20.0, 20.0, 20.0, 10.0],
            "low" => &[20.0, 20.0, 20.0, 10.0],
            "close" => &[20.0, 20.0, 20.0, 10.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();
        assert_eq!(exits.len(), 1);
        assert_eq!(exits[0].timestamp_ms, 4000);
        assert!(exits[0].reason.contains("CCI Momentum Sell"));

        Ok(())
    }
}
