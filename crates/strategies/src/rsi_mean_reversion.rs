use crate::indicators::{rsi, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RsiMeanReversionConfig {
    pub period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_pct: f64,
    pub symbol: String,
}

impl StrategyConfig for RsiMeanReversionConfig {}

pub struct RsiMeanReversion {
    config: RsiMeanReversionConfig,
}

impl RsiMeanReversion {
    pub fn new(config: RsiMeanReversionConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for RsiMeanReversion {
    fn name(&self) -> &str {
        "RsiMeanReversion"
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

        // Calculate RSI
        let rsi_series = rsi::calculate(data, self.config.period)?;
        let rsi_arr = rsi_series.f64()?;

        // Calculate SMA for Take Profit (Period = same as RSI period for mean reversion target)
        let sma_series = sma::calculate(data, self.config.period)?;
        let sma_arr = sma_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_pct_dec =
            Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let rsi_opt = rsi_arr.get(i);
            let sma_opt = sma_arr.get(i);

            if let (Some(price), Some(rsi_val)) = (price_opt, rsi_opt) {
                // Check for Exit (Overbought) - Stateless
                if rsi_val > self.config.overbought_threshold {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "RSI Overbought: {:.2} > {:.2}",
                            rsi_val, self.config.overbought_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Check for Entry (Oversold) - Stateless
                if rsi_val < self.config.oversold_threshold {
                    let sl = price * (one_dec - stop_loss_pct_dec);
                    let tp = sma_opt.unwrap_or(price.to_f64().unwrap_or(0.0) * 1.05); // Fallback to 5% if SMA missing

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Enter Long
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp),
                        reason: format!(
                            "RSI Oversold: {:.2} < {:.2}",
                            rsi_val, self.config.oversold_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: RsiMeanReversionConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_rsi_entry_exit_stateless() -> Result<()> {
        let config = RsiMeanReversionConfig {
            period: 2,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            stop_loss_pct: 0.1,
            symbol: "TEST".to_string(),
        };
        let strategy = RsiMeanReversion::new(config);

        // Construct data
        // 0: 100
        // 1: 90
        // 2: 80 (RSI < 30 -> Entry)
        // 3: 100
        // 4: 120 (RSI > 70 -> Exit)

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "close" => &[100.0, 90.0, 80.0, 100.0, 120.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Should have 2 signals: Entry at 3000, Exit at 5000.
        // Even if we process them in one go, they are independent.

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        assert_eq!(entries.len(), 1);
        assert_eq!(exits.len(), 1);

        let entry = entries[0];
        assert_eq!(entry.timestamp_ms, 3000);
        assert!(entry.reason.contains("RSI Oversold"));
        assert!(entry.take_profit.is_some());

        let exit = exits[0];
        assert_eq!(exit.timestamp_ms, 5000);
        assert!(exit.reason.contains("RSI Overbought"));

        Ok(())
    }

    #[tokio::test]
    async fn test_stateless_exit_only() -> Result<()> {
        let config = RsiMeanReversionConfig {
            period: 2,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            stop_loss_pct: 0.1,
            symbol: "TEST".to_string(),
        };
        let strategy = RsiMeanReversion::new(config);

        // Data that starts already high -> Overbought -> Exit
        // 0: 100
        // 1: 110
        // 2: 130 (RSI High -> Exit)

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000],
            "close" => &[100.0, 110.0, 130.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect ONLY Exit signal. No Entry.
        assert_eq!(signals.len(), 1);
        let signal = &signals[0];
        assert_eq!(signal.signal_type, SignalType::Exit);
        assert!(signal.reason.contains("RSI Overbought"));

        Ok(())
    }
}
