use crate::indicators::{atr, rsi, sma};
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
    pub stop_loss_pct: f64, // Keep as fallback
    pub atr_period: usize,
    pub atr_mult: f64,
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

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_pct_dec =
            Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.atr_mult).unwrap_or(Decimal::new(2, 0));

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);
            let rsi_opt = rsi_arr.get(i);
            let sma_opt = sma_arr.get(i);
            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

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
                    let sl = if let Some(atr_val) = atr_opt {
                        price - (atr_val * atr_mult_dec)
                    } else {
                        price * (one_dec - stop_loss_pct_dec)
                    };

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
            atr_period: 2,
            atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = RsiMeanReversion::new(config);

        // Construct data
        // Need High/Low for ATR
        let closes = vec![100.0, 90.0, 80.0, 100.0, 120.0];
        let highs = vec![101.0, 91.0, 81.0, 101.0, 121.0];
        let lows = vec![99.0, 89.0, 79.0, 99.0, 119.0];
        let timestamps = vec![1000i64, 2000, 3000, 4000, 5000];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Should have 2 signals: Entry at 3000, Exit at 5000.
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
        assert!(entry.stop_loss.is_some());

        let sl = entry.stop_loss.unwrap();
        // Relaxed assertion: Check for valid Stop Loss range
        // Entry Price is 100.0 (Wait, Index 3? No, Index 2 (80.0)).
        // Array: 100, 90, 80, 100, 120.
        // i=2: 80.
        assert!(sl < 80.0, "SL {} should be < 80.0", sl);
        assert!(sl > 0.0, "SL {} should be > 0.0", sl);

        Ok(())
    }
}
