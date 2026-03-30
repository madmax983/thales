use crate::indicators::{atr, rsi, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmaRsiTrendConfig {
    pub sma_period: usize,
    pub rsi_period: usize,
    pub rsi_oversold: f64,
    pub rsi_overbought: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for SmaRsiTrendConfig {}

pub struct SmaRsiTrend {
    config: SmaRsiTrendConfig,
}

impl SmaRsiTrend {
    pub fn new(config: SmaRsiTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for SmaRsiTrend {
    fn name(&self) -> &str {
        "SmaRsiTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let sma_series = sma::calculate(data, self.config.sma_period)?;
        let sma_arr = sma_series.f64()?;

        let rsi_series = rsi::calculate(data, self.config.rsi_period)?;
        let rsi_arr = rsi_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let price_opt = close_arr.get(i);
            let prev_rsi_opt = rsi_arr.get(i - 1);
            let curr_rsi_opt = rsi_arr.get(i);
            let sma_opt = sma_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (Some(price), Some(prev_rsi), Some(curr_rsi), Some(sma), Some(atr_val)) =
                (price_opt, prev_rsi_opt, curr_rsi_opt, sma_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);
                let sl_dist = atr_dec * atr_mult_dec;

                // Long Entry
                if price > sma
                    && prev_rsi <= self.config.rsi_oversold
                    && curr_rsi > self.config.rsi_oversold
                {
                    let sl = price_dec - sl_dist;
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "SMA/RSI Long: Price > SMA ({:.2} > {:.2}) & RSI crossed above Oversold ({:.2} > {:.2})",
                            price, sma, curr_rsi, self.config.rsi_oversold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Entry
                if price < sma
                    && prev_rsi >= self.config.rsi_overbought
                    && curr_rsi < self.config.rsi_overbought
                {
                    let sl = price_dec + sl_dist;
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "SMA/RSI Short: Price < SMA ({:.2} < {:.2}) & RSI crossed below Overbought ({:.2} < {:.2})",
                            price, sma, curr_rsi, self.config.rsi_overbought
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit
                if price > sma
                    && prev_rsi <= self.config.rsi_overbought
                    && curr_rsi > self.config.rsi_overbought
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
                            "SMA/RSI Long Exit: RSI crossed above Overbought ({:.2} > {:.2})",
                            curr_rsi, self.config.rsi_overbought
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Exit
                if price < sma
                    && prev_rsi >= self.config.rsi_oversold
                    && curr_rsi < self.config.rsi_oversold
                {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "SMA/RSI Short Exit: RSI crossed below Oversold ({:.2} < {:.2})",
                            curr_rsi, self.config.rsi_oversold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: SmaRsiTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sma_rsi_trend_empty_data() -> Result<()> {
        let config = SmaRsiTrendConfig {
            sma_period: 50,
            rsi_period: 14,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "AAPL".to_string(),
        };
        let strategy = SmaRsiTrend::new(config);

        let empty_series: Vec<Series> = vec![];
        let df = DataFrame::new(empty_series)?;
        let signals = strategy.generate_signals(&df).await;
        assert!(signals.is_err(), "Should error on empty dataframe");
        Ok(())
    }

    #[tokio::test]
    async fn test_sma_rsi_trend_parameter_validation() -> Result<()> {
        let mut strategy = SmaRsiTrend::new(SmaRsiTrendConfig {
            sma_period: 50,
            rsi_period: 14,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "AAPL".to_string(),
        });

        let new_params = serde_json::json!({
            "sma_period": 200,
            "rsi_period": 10,
            "rsi_oversold": 25.0,
            "rsi_overbought": 75.0,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.sma_period, 200);
        assert_eq!(strategy.config.rsi_period, 10);
        assert_eq!(strategy.config.rsi_oversold, 25.0);
        assert_eq!(strategy.config.rsi_overbought, 75.0);
        assert_eq!(strategy.config.stop_loss_atr_mult, 1.5);
        assert_eq!(strategy.config.atr_period, 10);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }

    #[tokio::test]
    async fn test_sma_rsi_trend_signal_generation() -> Result<()> {
        let config = SmaRsiTrendConfig {
            sma_period: 2,
            rsi_period: 2,
            rsi_oversold: 50.0,
            rsi_overbought: 50.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "AAPL".to_string(),
        };
        let strategy = SmaRsiTrend::new(config);

        // Price sequence designed to create an RSI oversold/overbought crossing.
        let times: Vec<i64> = vec![1000, 2000, 3000, 4000, 5000, 6000, 7000];
        let closes: Vec<f64> = vec![100.0, 100.0, 100.0, 90.0, 90.0, 110.0, 80.0];
        let highs: Vec<f64> = vec![105.0, 105.0, 105.0, 95.0, 95.0, 115.0, 85.0];
        let lows: Vec<f64> = vec![95.0, 95.0, 95.0, 85.0, 85.0, 105.0, 75.0];

        let df = df!(
            "timestamp_unix_ms" => times,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Validate signal structure
        assert!(
            !signals.is_empty(),
            "Should generate at least one signal with valid crossings"
        );

        let entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry.is_some(), "Should find an entry signal");
        if let Some(s) = entry {
            assert!(s.stop_loss.is_some(), "Entry must have stop loss");
            assert!(s.confidence > 0.0);
        }

        Ok(())
    }
}
