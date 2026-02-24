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
        let mut has_scaled_in = false;
        let mut has_scaled_out = false;
        let stop_loss_pct_dec = Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;
        let two_dec = Decimal::from(2);

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let rsi_opt = rsi_arr.get(i);

            if let (Some(price), Some(rsi_val)) = (price_opt, rsi_opt) {
                // Check Existing Position Logic first
                if let Some(entry) = entry_price {
                    // 1. Stop Loss Check
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
                        has_scaled_in = false;
                        has_scaled_out = false;
                        continue;
                    }

                    // 2. ScaleOut (Take Profit / Trim)
                    if rsi_val > self.config.overbought_threshold {
                        // If very high, full Exit
                        if rsi_val > self.config.overbought_threshold + 10.0 {
                             signals.push(Signal {
                                signal_type: SignalType::Exit,
                                symbol: self.config.symbol.clone(),
                                side: "sell".to_string(),
                                size_hint: "max".to_string(),
                                confidence: 0.9,
                                stop_loss: None,
                                take_profit: None,
                                reason: format!("RSI Extreme Overbought (Exit): {:.2} > {:.2}", rsi_val, self.config.overbought_threshold + 10.0),
                                timestamp_ms: timestamp,
                            });
                            entry_price = None;
                            has_scaled_in = false;
                            has_scaled_out = false;
                        } else if !has_scaled_out {
                            // Just Overbought, ScaleOut once
                            signals.push(Signal {
                                signal_type: SignalType::ScaleOut,
                                symbol: self.config.symbol.clone(),
                                side: "sell".to_string(),
                                size_hint: "100".to_string(),
                                confidence: 0.7,
                                stop_loss: None,
                                take_profit: None,
                                reason: format!("RSI Overbought (ScaleOut): {:.2} > {:.2}", rsi_val, self.config.overbought_threshold),
                                timestamp_ms: timestamp,
                            });
                            has_scaled_out = true;
                        }
                        continue;
                    }

                    // 3. ScaleIn (Add to position)
                    if rsi_val < self.config.oversold_threshold - 10.0 && !has_scaled_in {
                         let sl = price * (one_dec - stop_loss_pct_dec);
                         let tp = price * (one_dec + (two_dec * stop_loss_pct_dec));

                         signals.push(Signal {
                            signal_type: SignalType::ScaleIn,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.6,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                            reason: format!("RSI Extreme Oversold (ScaleIn): {:.2} < {:.2}", rsi_val, self.config.oversold_threshold - 10.0),
                            timestamp_ms: timestamp,
                        });
                        has_scaled_in = true;
                        continue;
                    }
                }

                // Check Entry Logic (No Position)
                if entry_price.is_none() {
                    if rsi_val < self.config.oversold_threshold {
                        let sl = price * (one_dec - stop_loss_pct_dec);
                        let tp = price * (one_dec + (two_dec * stop_loss_pct_dec));

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                            reason: format!("RSI Oversold (Entry): {:.2} < {:.2}", rsi_val, self.config.oversold_threshold),
                            timestamp_ms: timestamp,
                        });
                        entry_price = Some(price);
                        has_scaled_in = false;
                        has_scaled_out = false;
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
        // 2: 80 (Change -10). RSI=0. < 30. Entry (Buy).
        // 3: 100 (Change +20). RSI=66. Hold.
        // 4: 120 (Change +20). RSI=85. > 80. Exit (Sell).

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "close" => &[100.0, 90.0, 80.0, 100.0, 120.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        assert_eq!(signals.len(), 2);

        let entry = &signals[0];
        assert_eq!(entry.signal_type, SignalType::Entry);
        assert_eq!(entry.side, "buy");
        assert_eq!(entry.timestamp_ms, 3000);
        assert!(entry.take_profit.is_some()); // Verify TP added

        let exit = &signals[1];
        assert_eq!(exit.signal_type, SignalType::Exit);
        assert_eq!(exit.side, "sell");
        assert_eq!(exit.timestamp_ms, 5000);

        Ok(())
    }

    #[tokio::test]
    async fn test_scale_in_once() -> Result<()> {
        let config = RsiMeanReversionConfig {
            period: 2,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            stop_loss_pct: 0.1,
            symbol: "TEST".to_string(),
        };
        let strategy = RsiMeanReversion::new(config);

        // 0: 100
        // 1: 90
        // 2: 80. RSI 0. Entry (Buy).
        // 3: 75. Change -5. AvgGain 0. AvgLoss (10+5)/2=7.5. RSI 0. < 20. ScaleIn.
        // 4: 70. Change -5. RSI 0. < 20. Should NOT ScaleIn again.

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "close" => &[100.0, 90.0, 80.0, 75.0, 70.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect Entry, then ScaleIn. No second ScaleIn.
        // But wait, 70 might trigger Stop Loss? Entry 80. SL 72.
        // Price 75 -> OK. ScaleIn.
        // Price 70 -> SL Hit! (70 <= 72).
        // So third signal is Exit.

        assert_eq!(signals.len(), 3);
        assert_eq!(signals[0].signal_type, SignalType::Entry);
        assert_eq!(signals[1].signal_type, SignalType::ScaleIn);
        assert_eq!(signals[2].signal_type, SignalType::Exit);
        assert!(signals[2].reason.contains("Stop Loss"));

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
