//! The Chande Momentum Oscillator Strategy
//!
//! A mean reversion strategy using the CMO.
//!
use crate::indicators::{atr, cmo};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmoMeanReversionConfig {
    pub cmo_period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for CmoMeanReversionConfig {}

pub struct CmoMeanReversion {
    config: CmoMeanReversionConfig,
}

impl CmoMeanReversion {
    pub fn new(config: CmoMeanReversionConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for CmoMeanReversion {
    fn name(&self) -> &str {
        "CmoMeanReversion"
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

        // Calculate CMO
        let cmo_series = cmo::calculate(data, self.config.cmo_period)?;
        let cmo_arr = cmo_series.f64()?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let price_opt = close_arr.get(i);
            let prev_price_opt = close_arr.get(i - 1);
            let cmo_opt = cmo_arr.get(i);
            let prev_cmo_opt = cmo_arr.get(i - 1);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(price),
                Some(_prev_price),
                Some(cmo_val),
                Some(prev_cmo_val),
                Some(atr_val),
            ) = (price_opt, prev_price_opt, cmo_opt, prev_cmo_opt, atr_opt)
            {
                // Mean Reversion Entry (Long)
                // Buy when CMO crosses above oversold threshold
                if prev_cmo_val <= self.config.oversold_threshold
                    && cmo_val > self.config.oversold_threshold
                {
                    let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                    let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);
                    let sl = price_dec - (atr_dec * atr_mult_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "CMO Cross Up: {:.2} > {:.2}",
                            cmo_val, self.config.oversold_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Mean Reversion Exit (Long)
                // Sell when CMO crosses below overbought threshold
                if prev_cmo_val >= self.config.overbought_threshold
                    && cmo_val < self.config.overbought_threshold
                {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "CMO Cross Down: {:.2} < {:.2}",
                            cmo_val, self.config.overbought_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: CmoMeanReversionConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cmo_mean_reversion_empty_data() -> Result<()> {
        let config = CmoMeanReversionConfig {
            cmo_period: 9,
            oversold_threshold: -50.0,
            overbought_threshold: 50.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "AAPL".to_string(),
        };
        let strategy = CmoMeanReversion::new(config);

        let empty_series: Vec<Series> = vec![];
        let df = DataFrame::new(empty_series)?;
        let signals = strategy.generate_signals(&df).await;
        assert!(signals.is_err(), "Should error on empty dataframe");
        Ok(())
    }

    #[tokio::test]
    async fn test_cmo_mean_reversion_signal_generation() -> Result<()> {
        let config = CmoMeanReversionConfig {
            cmo_period: 2, // very short to trigger easily
            oversold_threshold: -50.0,
            overbought_threshold: 50.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2, // short ATR
            symbol: "AAPL".to_string(),
        };
        let strategy = CmoMeanReversion::new(config);

        // create price sequence to trigger:
        // down trend to create negative CMO < -50
        // then up trend to cross > -50
        // then up trend to > 50
        // then down trend to cross < 50

        // 0: 100
        // 1: 90
        // 2: 80 (cmo < -50)
        // 3: 85 (cmo > -50, Entry)
        // 4: 100 (cmo > 50)
        // 5: 90 (cmo < 50, Exit)

        let times: Vec<i64> = vec![1000, 2000, 3000, 4000, 5000, 6000];
        let closes: Vec<f64> = vec![100.0, 90.0, 80.0, 85.0, 100.0, 90.0];
        let highs: Vec<f64> = vec![105.0, 95.0, 85.0, 90.0, 105.0, 95.0];
        let lows: Vec<f64> = vec![95.0, 85.0, 75.0, 80.0, 95.0, 85.0];

        let df = df!(
            "timestamp_unix_ms" => times,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // We expect an entry and an exit
        let entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry.is_some(), "Should have generated an entry signal");
        let entry = entry.unwrap();
        assert_eq!(entry.side, "buy");
        assert_eq!(entry.size_hint, "100");
        assert!(entry.stop_loss.is_some());

        let exit = signals.iter().find(|s| s.signal_type == SignalType::Exit);
        assert!(exit.is_some(), "Should have generated an exit signal");
        let exit = exit.unwrap();
        assert_eq!(exit.side, "sell");
        assert_eq!(exit.size_hint, "max");
        assert!(exit.stop_loss.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_cmo_mean_reversion_update_params() -> Result<()> {
        let mut strategy = CmoMeanReversion::new(CmoMeanReversionConfig {
            cmo_period: 9,
            oversold_threshold: -50.0,
            overbought_threshold: 50.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "AAPL".to_string(),
        });

        let new_params = serde_json::json!({
            "cmo_period": 14,
            "oversold_threshold": -40.0,
            "overbought_threshold": 60.0,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.cmo_period, 14);
        assert_eq!(strategy.config.oversold_threshold, -40.0);
        assert_eq!(strategy.config.overbought_threshold, 60.0);
        assert_eq!(strategy.config.stop_loss_atr_mult, 1.5);
        assert_eq!(strategy.config.atr_period, 10);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
