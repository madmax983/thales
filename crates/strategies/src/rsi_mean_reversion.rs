use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig};
use crate::indicators::rsi;
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

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let rsi_series = rsi::calculate(data, self.config.period)?;
        let rsi_arr = rsi_series.f64()?;

        let mut signals = Vec::new();
        let mut entry_price: Option<Decimal> = None;
        let stop_loss_pct_dec = Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let rsi_opt = rsi_arr.get(i);

            if let (Some(price), Some(rsi_val)) = (price_opt, rsi_opt) {
                // Check for Exit first
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

                    // Overbought Exit
                    if rsi_val > self.config.overbought_threshold {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            stop_loss: None,
                            take_profit: None,
                            reason: format!("RSI Overbought: {:.2} > {:.2}", rsi_val, self.config.overbought_threshold),
                            timestamp_ms: timestamp,
                        });
                        entry_price = None;
                        continue;
                    }
                }

                // Check for Entry
                if entry_price.is_none() {
                    // Oversold Entry
                    if rsi_val < self.config.oversold_threshold {
                         let sl = price * (one_dec - stop_loss_pct_dec);

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: None,
                            reason: format!("RSI Oversold: {:.2} < {:.2}", rsi_val, self.config.oversold_threshold),
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
    async fn test_rsi_entry_exit() -> Result<()> {
        let config = RsiMeanReversionConfig {
            period: 2,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            stop_loss_pct: 0.1,
            symbol: "TEST".to_string(),
        };
        let strategy = RsiMeanReversion::new(config);

        // Construct data to trigger RSI
        // Period 2.
        // 0: 100
        // 1: 90 (Change -10)
        // 2: 80 (Change -10). AvgGain=0, AvgLoss=10. RSI=0. < 30. ENTRY.
        // 3: 100 (Change +20). Gain 20. AvgGain=(0*1+20)/2=10. AvgLoss=(10*1+0)/2=5. RS=2. RSI=100-33=66. Hold.
        // 4: 120 (Change +20). Gain 20. AvgGain=(10*1+20)/2=15. AvgLoss=(5*1+0)/2=2.5. RS=6. RSI=100-14=85. > 70. EXIT.

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "close" => &[100.0, 90.0, 80.0, 100.0, 120.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        assert_eq!(signals.len(), 2);

        let entry = &signals[0];
        assert_eq!(entry.signal_type, SignalType::Entry);
        assert_eq!(entry.timestamp_ms, 3000); // Index 2
        assert!(entry.reason.contains("RSI Oversold"));

        let exit = &signals[1];
        assert_eq!(exit.signal_type, SignalType::Exit);
        assert_eq!(exit.timestamp_ms, 5000); // Index 4
        assert!(exit.reason.contains("RSI Overbought"));

        Ok(())
    }

    #[tokio::test]
    async fn test_stop_loss() -> Result<()> {
        let config = RsiMeanReversionConfig {
            period: 2,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            stop_loss_pct: 0.1, // 10% SL
            symbol: "TEST".to_string(),
        };
        let strategy = RsiMeanReversion::new(config);

        // 0: 100
        // 1: 90
        // 2: 80 (Entry). RSI=0. Price=80. SL Price = 72.
        // 3: 70 (SL Hit). 70 < 72. Exit.

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "close" => &[100.0, 90.0, 80.0, 70.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        assert_eq!(signals.len(), 2);
        assert_eq!(signals[0].signal_type, SignalType::Entry);

        let exit = &signals[1];
        assert_eq!(exit.signal_type, SignalType::Exit);
        assert_eq!(exit.timestamp_ms, 4000);
        assert!(exit.reason.contains("Stop Loss"));

        Ok(())
    }

    #[tokio::test]
    async fn test_not_enough_data() -> Result<()> {
         let config = RsiMeanReversionConfig {
            period: 14,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            stop_loss_pct: 0.1,
            symbol: "TEST".to_string(),
        };
        let strategy = RsiMeanReversion::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000],
            "close" => &[100.0, 100.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());

        Ok(())
    }
}
