use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSurgeReversalConfig {
    pub rsi_period: usize,
    pub vol_short_period: usize,
    pub vol_long_period: usize,
    pub rsi_oversold: f64,
    pub rsi_overbought: f64,
    pub vol_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for VolumeSurgeReversalConfig {}

use crate::indicators::{atr, rsi, volume_oscillator};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

pub struct VolumeSurgeReversal {
    config: VolumeSurgeReversalConfig,
}

impl VolumeSurgeReversal {
    pub fn new(config: VolumeSurgeReversalConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for VolumeSurgeReversal {
    fn name(&self) -> &str {
        "VolumeSurgeReversal"
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

        let rsi_series = rsi::calculate(data, self.config.rsi_period)?;
        let rsi_arr = rsi_series.f64()?;

        let vol_osc_series = volume_oscillator::calculate(data, self.config.vol_short_period, self.config.vol_long_period)?;
        let vol_osc_arr = vol_osc_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult_opt = Decimal::from_f64_retain(self.config.stop_loss_atr_mult);
        let sl_mult = if let Some(m) = sl_mult_opt { m } else { Decimal::ZERO };
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = if let Some(t) = time_arr.get(i) { t } else { 0 };
            let price_opt = close_arr.get(i);
            let rsi_curr_opt = rsi_arr.get(i);
            let vol_curr_opt = vol_osc_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (Some(price), Some(rsi_val), Some(vol_val), Some(atr_val)) = (
                price_opt,
                rsi_curr_opt,
                vol_curr_opt,
                atr_opt,
            ) {
                let price_dec = if let Some(p) = Decimal::from_f64_retain(price) { p } else { Decimal::ZERO };
                let atr_dec = if let Some(a) = Decimal::from_f64_retain(atr_val) { a } else { Decimal::ZERO };

                // Long Entry: RSI oversold AND Volume Surge
                if rsi_val < self.config.rsi_oversold && vol_val > self.config.vol_threshold {
                    let sl = price_dec - (atr_dec * sl_mult);
                    let risk = price_dec - sl;
                    let tp = price_dec + (risk * two_dec);
                    let sl_f64 = if let Some(s) = sl.to_f64() { s } else { 0.0 };
                    let tp_f64 = if let Some(t) = tp.to_f64() { t } else { 0.0 };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl_f64),
                        take_profit: Some(tp_f64),
                        reason: format!(
                            "Volume Surge Reversal: RSI {:.2} < {:.2} AND VolOsc {:.2} > {:.2}",
                            rsi_val, self.config.rsi_oversold, vol_val, self.config.vol_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Short Entry: RSI overbought AND Volume Surge
                else if rsi_val > self.config.rsi_overbought && vol_val > self.config.vol_threshold {
                    let sl = price_dec + (atr_dec * sl_mult);
                    let risk = sl - price_dec;
                    let tp = price_dec - (risk * two_dec);
                    let sl_f64 = if let Some(s) = sl.to_f64() { s } else { 0.0 };
                    let tp_f64 = if let Some(t) = tp.to_f64() { t } else { 0.0 };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl_f64),
                        take_profit: Some(tp_f64),
                        reason: format!(
                            "Volume Surge Reversal: RSI {:.2} > {:.2} AND VolOsc {:.2} > {:.2}",
                            rsi_val, self.config.rsi_overbought, vol_val, self.config.vol_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: VolumeSurgeReversalConfig = serde_json::from_value(params)?;
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
        let config = VolumeSurgeReversalConfig {
            rsi_period: 14,
            vol_short_period: 14,
            vol_long_period: 28,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            vol_threshold: 20.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = VolumeSurgeReversal::new(config);
        let empty_df = DataFrame::empty();
        let signals = strategy.generate_signals(&empty_df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = VolumeSurgeReversal::new(VolumeSurgeReversalConfig {
            rsi_period: 14,
            vol_short_period: 14,
            vol_long_period: 28,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            vol_threshold: 20.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "rsi_period": 7,
            "vol_short_period": 5,
            "vol_long_period": 10,
            "rsi_oversold": 20.0,
            "rsi_overbought": 80.0,
            "vol_threshold": 15.0,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.rsi_period, 7);
        assert_eq!(strategy.config.rsi_oversold, 20.0);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_signals() -> Result<()> {
        let config = VolumeSurgeReversalConfig {
            rsi_period: 2,
            vol_short_period: 2,
            vol_long_period: 3,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            vol_threshold: 5.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = VolumeSurgeReversal::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000],
            "open"  => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "high"  => &[11.0, 12.0, 13.0, 14.0, 15.0, 12.0, 10.0],
            "low"   => &[9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0],
            "close" => &[10.0, 10.0, 10.0, 5.0, 20.0, 10.0, 8.0],
            "volume"=> &[100.0, 100.0, 100.0, 500.0, 1000.0, 100.0, 100.0]
        )?;

        let _signals = strategy.generate_signals(&df).await?;

        // We aren't testing the exact math of the RSI and VolumeOscillator here,
        // just that the signal generation path works without crashing.
        // It's acceptable for it to be empty or contain signals depending on the mocked values.
        Ok(())
    }
}
