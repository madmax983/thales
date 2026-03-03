use crate::indicators::{atr, sma, zscore};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZScoreMeanReversionConfig {
    pub period: usize,
    pub entry_threshold: f64,
    pub exit_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for ZScoreMeanReversionConfig {}

pub struct ZScoreMeanReversion {
    pub config: ZScoreMeanReversionConfig,
}

impl ZScoreMeanReversion {
    pub fn new(config: ZScoreMeanReversionConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for ZScoreMeanReversion {
    fn name(&self) -> &str {
        "ZScoreMeanReversion"
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

        // Calculate Z-Score
        let zscore_series = zscore::calculate(data, self.config.period)?;
        let zscore_arr = zscore_series.f64()?;

        // Calculate SMA for mean reversion target estimation
        let sma_series = sma::calculate(data, self.config.period)?;
        let sma_arr = sma_series.f64()?;

        // Calculate ATR for stop loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));

        let entry_threshold = self.config.entry_threshold;
        let exit_threshold = self.config.exit_threshold;
        let neg_entry_threshold = -entry_threshold;
        let neg_exit_threshold = -exit_threshold;

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);

            let prev_zscore_opt = zscore_arr.get(i - 1);
            let zscore_opt = zscore_arr.get(i);

            let sma_opt = sma_arr.get(i);
            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(price), Some(prev_z), Some(curr_z)) =
                (price_opt, prev_zscore_opt, zscore_opt)
            {
                // Determine sl dynamically if available
                let atr_dec = atr_opt.unwrap_or(Decimal::ZERO);

                // Long Entry: Z-Score crosses below -entry_threshold
                if prev_z >= neg_entry_threshold && curr_z < neg_entry_threshold {
                    let sl = price - (atr_dec * atr_mult_dec);
                    let tp = sma_opt.unwrap_or(price.to_f64().unwrap_or(0.0) * 1.02);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp),
                        reason: format!(
                            "ZScore Oversold: {:.2} < {:.2}",
                            curr_z, neg_entry_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Short Entry: Z-Score crosses above entry_threshold
                else if prev_z <= entry_threshold && curr_z > entry_threshold {
                    let sl = price + (atr_dec * atr_mult_dec);
                    let tp = sma_opt.unwrap_or(price.to_f64().unwrap_or(0.0) * 0.98);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp),
                        reason: format!(
                            "ZScore Overbought: {:.2} > {:.2}",
                            curr_z, entry_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit: Z-Score crosses above exit_threshold
                if prev_z <= exit_threshold && curr_z > exit_threshold {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "ZScore Reverted (Long Exit): {:.2} > {:.2}",
                            curr_z, exit_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Exit: Z-Score crosses below -exit_threshold
                if prev_z >= neg_exit_threshold && curr_z < neg_exit_threshold {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "ZScore Reverted (Short Exit): {:.2} < {:.2}",
                            curr_z, neg_exit_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: ZScoreMeanReversionConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_zscore_signals() -> Result<()> {
        let config = ZScoreMeanReversionConfig {
            period: 3,
            entry_threshold: 1.0,
            exit_threshold: 0.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 3,
            symbol: "TEST".to_string(),
        };
        let strategy = ZScoreMeanReversion::new(config);

        // Data that induces Z-score movements
        let timestamps = vec![1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000];
        let closes = vec![10.0, 10.0, 10.0, 5.0, 10.0, 15.0, 10.0, 10.0];
        let highs = vec![11.0, 11.0, 11.0, 6.0, 11.0, 16.0, 11.0, 11.0];
        let lows = vec![9.0, 9.0, 9.0, 4.0, 9.0, 14.0, 9.0, 9.0];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect long entry at index 3 (price drops to 5.0)
        let entries_long: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();
        assert!(entries_long.len() > 0, "Expected a Long Entry");

        // Expect long exit around index 4 (price reverts to 10.0)
        let exits_long: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit && s.side == "sell")
            .collect();
        assert!(exits_long.len() > 0, "Expected a Long Exit");

        // Expect short entry around index 5 (price shoots to 15.0)
        let entries_short: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "sell")
            .collect();
        assert!(entries_short.len() > 0, "Expected a Short Entry");

        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let mut strategy = ZScoreMeanReversion::new(ZScoreMeanReversionConfig {
            period: 14,
            entry_threshold: 2.0,
            exit_threshold: 0.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "period": 20,
            "entry_threshold": 2.5,
            "exit_threshold": 0.5,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 20,
            "symbol": "NEW_TEST"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.period, 20);
        assert_eq!(strategy.config.entry_threshold, 2.5);
        assert_eq!(strategy.config.symbol, "NEW_TEST");

        Ok(())
    }
}
