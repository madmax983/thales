use crate::indicators::mfi;
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneyFlowIndexConfig {
    pub period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_pct: f64,
    pub symbol: String,
}

impl StrategyConfig for MoneyFlowIndexConfig {}

pub struct MoneyFlowIndex {
    config: MoneyFlowIndexConfig,
}

impl MoneyFlowIndex {
    pub fn new(config: MoneyFlowIndexConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for MoneyFlowIndex {
    fn name(&self) -> &str {
        "MoneyFlowIndex"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate MFI
        let mfi_series = mfi::calculate(data, self.config.period)?;
        let mfi_arr = mfi_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_pct_dec =
            Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let mfi_opt = mfi_arr.get(i);

            if let (Some(price), Some(mfi_val)) = (price_opt, mfi_opt) {
                // Check for Exit (Overbought) - Stateless
                if mfi_val > self.config.overbought_threshold {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "MFI Overbought: {:.2} > {:.2}",
                            mfi_val, self.config.overbought_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Check for Entry (Oversold) - Stateless
                if mfi_val < self.config.oversold_threshold {
                    let sl = price * (one_dec - stop_loss_pct_dec);
                    // Take profit: Simple risk:reward 1:2 or fixed?
                    // Let's use a simple 2x Stop Loss distance for TP target
                    let risk = price - sl;
                    let tp = price + (risk * Decimal::from(2));

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Enter Long
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "MFI Oversold: {:.2} < {:.2}",
                            mfi_val, self.config.oversold_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: MoneyFlowIndexConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_mfi_entry_exit_stateless() -> Result<()> {
        let config = MoneyFlowIndexConfig {
            period: 2,
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            stop_loss_pct: 0.05,
            symbol: "TEST".to_string(),
        };
        let strategy = MoneyFlowIndex::new(config);

        // Construct data
        // We need MFI calculation to produce low/high values.
        // MFI uses Typical Price and Volume.

        // i=0: TP=10. V=100.
        // i=1: TP=11. V=100. Pos=1100. Neg=0.
        // i=2: TP=12. V=100. Pos=1200. Neg=0. Window [1,2]. SumPos=2300. SumNeg=0. MFI=100. -> Exit
        // i=3: TP=11. V=100. Pos=0. Neg=1100. Window [2,3]. SumPos=1200. SumNeg=1100. MFI=52.
        // i=4: TP=5. V=1000. Pos=0. Neg=5000. Window [3,4]. SumPos=0. SumNeg=6100. MFI=0. -> Entry

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "high" => &[10.0, 11.0, 12.0, 11.0, 5.0],
            "low" => &[10.0, 11.0, 12.0, 11.0, 5.0],
            "close" => &[10.0, 11.0, 12.0, 11.0, 5.0],
            "volume" => &[100.0, 100.0, 100.0, 100.0, 1000.0]
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

        // MFI at i=2 is 100.0 (>80) -> Exit
        assert_eq!(exits.len(), 1);
        let exit = exits[0];
        assert_eq!(exit.timestamp_ms, 3000);
        assert!(exit.reason.contains("MFI Overbought"));
        assert!(exit.reason.contains("100"));

        // MFI at i=4 is 0.0 (<20) -> Entry
        assert_eq!(entries.len(), 1);
        let entry = entries[0];
        assert_eq!(entry.timestamp_ms, 5000);
        assert!(entry.reason.contains("MFI Oversold"));
        assert!(entry.stop_loss.is_some());

        Ok(())
    }
}
