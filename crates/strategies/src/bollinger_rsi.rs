use crate::indicators::{atr, bollinger_bands, rsi};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BollingerRsiConfig {
    pub bb_period: usize,
    pub bb_std_dev: f64,
    pub rsi_period: usize,
    pub rsi_oversold: f64,
    pub rsi_overbought: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for BollingerRsiConfig {}

pub struct BollingerRsiMeanReversion {
    config: BollingerRsiConfig,
}

impl BollingerRsiMeanReversion {
    pub fn new(config: BollingerRsiConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for BollingerRsiMeanReversion {
    fn name(&self) -> &str {
        "BollingerRsiMeanReversion"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate Bollinger Bands
        let (_, lower_bb, upper_bb) =
            bollinger_bands::calculate(data, self.config.bb_period, self.config.bb_std_dev)?;
        let lower_bb_arr = lower_bb.f64()?;
        let upper_bb_arr = upper_bb.f64()?;

        // Calculate RSI
        let rsi_series = rsi::calculate(data, self.config.rsi_period)?;
        let rsi_arr = rsi_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));

        let mut in_position = false;
        let mut current_stop_loss = 0.0;

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);
            let lower_bb_opt = lower_bb_arr.get(i);
            let upper_bb_opt = upper_bb_arr.get(i);
            let rsi_opt = rsi_arr.get(i);
            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(price), Some(lower), Some(upper), Some(rsi_val)) =
                (price_opt, lower_bb_opt, upper_bb_opt, rsi_opt)
            {
                let price_f64 = price.to_f64().unwrap_or(0.0);

                if !in_position {
                    // Check for Entry (Price < Lower Band AND RSI < Oversold)
                    if price_f64 < lower && rsi_val < self.config.rsi_oversold {
                        let sl = if let Some(atr_val) = atr_opt {
                            (price - (atr_val * atr_mult_dec)).to_f64().unwrap_or(0.0)
                        } else {
                            price_f64 * 0.95 // 5% fallback
                        };

                        current_stop_loss = sl;
                        in_position = true;

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl),
                            take_profit: None,
                            reason: format!(
                                "BB & RSI Oversold: Price {:.2} < Lower {:.2}, RSI {:.2} < {:.2}",
                                price_f64, lower, rsi_val, self.config.rsi_oversold
                            ),
                            timestamp_ms: timestamp,
                        });
                    }
                } else {
                    // Check for Exit
                    let mut exit_reason = None;

                    if price_f64 <= current_stop_loss {
                        exit_reason = Some(format!("Stop Loss Hit at {:.2}", price_f64));
                    } else if price_f64 > upper {
                        exit_reason =
                            Some(format!("Price {:.2} > Upper BB {:.2}", price_f64, upper));
                    } else if rsi_val > self.config.rsi_overbought {
                        exit_reason = Some(format!(
                            "RSI Overbought: {:.2} > {:.2}",
                            rsi_val, self.config.rsi_overbought
                        ));
                    }

                    if let Some(reason) = exit_reason {
                        in_position = false;
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            stop_loss: None,
                            take_profit: None,
                            reason,
                            timestamp_ms: timestamp,
                        });
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: BollingerRsiConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let config = BollingerRsiConfig {
            bb_period: 20,
            bb_std_dev: 2.0,
            rsi_period: 14,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = BollingerRsiMeanReversion::new(config);
        let df = DataFrame::empty();
        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_bollinger_rsi_signals() -> Result<()> {
        let config = BollingerRsiConfig {
            bb_period: 2,
            bb_std_dev: 1.0,
            rsi_period: 2,
            rsi_oversold: 50.0,
            rsi_overbought: 50.0,
            stop_loss_atr_mult: 1.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = BollingerRsiMeanReversion::new(config);

        // Prices to trigger oversold (price drop) and then overbought (price rise)
        // Note: RSI(2) needs 3 bars. BB(2) needs 2 bars. ATR(2) needs 3 bars.
        // Array lengths should be enough.
        let closes = vec![100.0, 100.0, 100.0, 100.0, 10.0, 200.0];
        let highs = vec![101.0, 101.0, 101.0, 101.0, 11.0, 201.0];
        let lows = vec![99.0, 99.0, 99.0, 99.0, 9.0, 199.0];
        let timestamps = vec![1000i64, 2000, 3000, 4000, 5000, 6000];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows,
            "open" => vec![100.0, 100.0, 100.0, 100.0, 100.0, 100.0],
            "volume" => vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // We expect an entry when price drops to 10 (RSI should be low, below BB)
        // And an exit when price jumps to 200.
        assert!(!signals.is_empty());

        let entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry.is_some());

        let exit = signals.iter().find(|s| s.signal_type == SignalType::Exit);
        assert!(exit.is_some());

        if let Some(e) = entry {
            assert!(e.stop_loss.is_some());
            assert_eq!(e.side, "buy");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let mut strategy = BollingerRsiMeanReversion::new(BollingerRsiConfig {
            bb_period: 20,
            bb_std_dev: 2.0,
            rsi_period: 14,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "bb_period": 10,
            "bb_std_dev": 1.5,
            "rsi_period": 7,
            "rsi_oversold": 20.0,
            "rsi_overbought": 80.0,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.bb_period, 10);
        assert_eq!(strategy.config.rsi_period, 7);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
