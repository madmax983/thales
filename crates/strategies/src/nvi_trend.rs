use crate::indicators::{atr, nvi, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::{bail, Result};
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NviTrendConfig {
    pub nvi_sma_period: usize,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}

impl Default for NviTrendConfig {
    fn default() -> Self {
        Self {
            nvi_sma_period: 255,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for NviTrendConfig {}

pub struct NviTrend {
    config: NviTrendConfig,
}

impl NviTrend {
    pub fn new(config: NviTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for NviTrend {
    fn name(&self) -> &str {
        "NviTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            bail!("Data cannot be empty");
        }

        let close_series = data.column("close")?;
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate NVI
        let nvi_series = nvi::calculate(data)?;
        let nvi_arr = nvi_series.f64()?;

        // Calculate SMA of NVI (Signal line)
        // We have to put NVI in a dataframe for SMA to read the "close" column,
        let mut nvi_cloned = nvi_series.clone();
        nvi_cloned.rename("close");
        let temp_df = DataFrame::new(vec![nvi_cloned])?;
        let signal_series = sma::calculate(&temp_df, self.config.nvi_sma_period)?;
        let signal_arr = signal_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let nvi_curr = nvi_arr.get(i);
            let nvi_prev = nvi_arr.get(i - 1);

            let sig_curr = signal_arr.get(i);
            let sig_prev = signal_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(n_curr),
                Some(n_prev),
                Some(s_curr),
                Some(s_prev),
                Some(price),
                Some(atr_val),
            ) = (nvi_curr, nvi_prev, sig_curr, sig_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Long Entry
                if n_prev <= s_prev && n_curr > s_curr {
                    let sl = price_dec - (atr_dec * sl_mult);
                    let risk = price_dec - sl;
                    let tp = price_dec + (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: "NVI crossed above Signal Line (Bullish Trend)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Entry
                if n_prev >= s_prev && n_curr < s_curr {
                    let sl = price_dec + (atr_dec * sl_mult);
                    let risk = sl - price_dec;
                    let tp = price_dec - (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: "NVI crossed below Signal Line (Bearish Trend)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Long Exit
                if n_prev >= s_prev && n_curr < s_curr {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "NVI crossed below Signal Line (Long Exit)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Short Exit
                if n_prev <= s_prev && n_curr > s_curr {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "NVI crossed above Signal Line (Short Exit)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: NviTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let config = NviTrendConfig::default();
        let strategy = NviTrend::new(config);
        let df = DataFrame::default();
        let res = strategy.generate_signals(&df).await;
        assert!(res.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = NviTrend::new(NviTrendConfig::default());
        let new_params = serde_json::json!({
            "nvi_sma_period": 100,
            "atr_period": 10,
            "stop_loss_atr_mult": 1.5,
            "symbol": "BTCUSD"
        });
        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.nvi_sma_period, 100);
        assert_eq!(strategy.config.symbol, "BTCUSD");
        Ok(())
    }

    #[tokio::test]
    async fn test_signal_generation() -> Result<()> {
        let config = NviTrendConfig {
            nvi_sma_period: 2,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            symbol: "AAPL".to_string(),
        };
        let strategy = NviTrend::new(config);

        let times: Vec<i64> = vec![1000, 2000, 3000, 4000, 5000, 6000, 7000];
        let closes: Vec<f64> = vec![100.0, 105.0, 102.0, 104.0, 101.0, 103.0, 100.0];
        let highs: Vec<f64> = vec![105.0, 110.0, 105.0, 105.0, 105.0, 105.0, 105.0];
        let lows: Vec<f64> = vec![95.0, 95.0, 95.0, 100.0, 100.0, 100.0, 100.0];
        let volumes: Vec<f64> = vec![1000.0, 1200.0, 900.0, 1100.0, 800.0, 1300.0, 700.0];

        let mut df = df!(
            "timestamp_unix_ms" => times,
            "close" => closes,
            "high" => highs,
            "low" => lows,
            "volume" => volumes
        )?;

        df.try_apply("close", |s| s.cast(&DataType::Float64))?;
        df.try_apply("volume", |s| s.cast(&DataType::Float64))?;

        let signals = strategy.generate_signals(&df).await?;

        let entry_signals = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect::<Vec<_>>();
        let exit_signals = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect::<Vec<_>>();

        assert!(
            !entry_signals.is_empty(),
            "Should generate entry signals on crossover"
        );
        assert!(
            !exit_signals.is_empty(),
            "Should generate exit signals on crossover"
        );

        Ok(())
    }
}
