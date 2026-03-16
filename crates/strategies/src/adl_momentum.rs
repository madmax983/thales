use crate::indicators::{adl, atr, ema};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdlMomentumConfig {
    pub adl_sma_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for AdlMomentumConfig {}

impl AdlMomentumConfig {
    pub fn validate(&self) -> Result<()> {
        if self.adl_sma_period == 0 {
            anyhow::bail!("adl_sma_period must be > 0");
        }
        if self.atr_period == 0 {
            anyhow::bail!("atr_period must be > 0");
        }
        if self.stop_loss_atr_mult <= 0.0 {
            anyhow::bail!("stop_loss_atr_mult must be > 0.0");
        }
        if self.symbol.is_empty() {
            anyhow::bail!("symbol must not be empty");
        }
        Ok(())
    }
}

pub struct AdlMomentum {
    config: AdlMomentumConfig,
}

impl AdlMomentum {
    pub fn new(config: AdlMomentumConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for AdlMomentum {
    fn name(&self) -> &str {
        "AdlMomentum"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        self.config.validate()?;

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate ADL
        let adl_series = adl::calculate(data)?;

        // Rename ADL series to "close" to pass it to standard indicators like EMA
        let mut adl_df = DataFrame::new(vec![adl_series.clone()])?;
        adl_df.rename("adl", "close")?;

        // Calculate EMA of ADL
        let adl_ema_series = ema::calculate(&adl_df, self.config.adl_sma_period)?;
        let adl_ema_arr = adl_ema_series.f64()?;

        let adl_arr = adl_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));

        // Iterate through data starting from 1 to check for crossover
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);

            let adl_curr_opt = adl_arr.get(i);
            let adl_prev_opt = adl_arr.get(i - 1);
            let ema_curr_opt = adl_ema_arr.get(i);
            let ema_prev_opt = adl_ema_arr.get(i - 1);

            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(price), Some(adl_curr), Some(adl_prev), Some(ema_curr), Some(ema_prev)) = (
                price_opt,
                adl_curr_opt,
                adl_prev_opt,
                ema_curr_opt,
                ema_prev_opt,
            ) {
                // Long Entry: ADL crosses above its EMA
                if adl_prev <= ema_prev && adl_curr > ema_curr {
                    let sl = if let Some(atr_val) = atr_opt {
                        price - (atr_val * atr_mult_dec)
                    } else {
                        price * Decimal::from_f64_retain(0.98).unwrap_or(Decimal::ONE)
                    };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "ADL ({:.2}) crossed above EMA ({:.2})",
                            adl_curr, ema_curr
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Long Exit: ADL crosses below its EMA
                else if adl_prev >= ema_prev && adl_curr < ema_curr {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "ADL ({:.2}) crossed below EMA ({:.2})",
                            adl_curr, ema_curr
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: AdlMomentumConfig = serde_json::from_value(params)?;
        new_config.validate()?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_df() -> Result<DataFrame> {
        let timestamps: Vec<i64> = (1..=20).map(|i| i * 1000).collect();
        // create a series where price moves to trigger ADL, which then triggers EMA crossover
        let closes: Vec<f64> = vec![
            10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, // indices 0-9
            20.0, 20.0, 20.0, 20.0, 20.0, // indices 10-14, up
            5.0, 5.0, 5.0, 5.0, 5.0, // indices 15-19, down
        ];

        // To make ADL change, we need close != (high+low)/2
        // If close is closer to high, MFM is positive
        // If close is closer to low, MFM is negative
        let highs: Vec<f64> = closes.iter().map(|&c| c + 2.0).collect();
        let lows: Vec<f64> = closes.iter().map(|&c| c - 1.0).collect(); // high-low = 3.0. close is closer to low.

        let mut volumes: Vec<f64> = vec![100.0; 20];

        // Let's modify closes/highs/lows so ADL moves up then down
        let mut test_closes = closes.clone();
        let mut test_highs = highs.clone();
        let mut test_lows = lows.clone();

        for i in 10..15 {
            // Trend up: close at high
            test_closes[i] = 20.0;
            test_highs[i] = 20.0;
            test_lows[i] = 18.0;
            volumes[i] = 200.0;
        }
        for i in 15..20 {
            // Trend down: close at low
            test_closes[i] = 5.0;
            test_highs[i] = 7.0;
            test_lows[i] = 5.0;
            volumes[i] = 200.0;
        }

        Ok(df!(
            "timestamp_unix_ms" => timestamps,
            "close" => test_closes,
            "high" => test_highs,
            "low" => test_lows,
            "volume" => volumes
        )?)
    }

    #[tokio::test]
    async fn test_crossover_signals() -> Result<()> {
        let config = AdlMomentumConfig {
            adl_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = AdlMomentum::new(config);
        let df = create_test_df()?;

        let signals = strategy.generate_signals(&df).await?;

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        assert!(!entries.is_empty(), "Should have generated an entry signal");
        assert!(!exits.is_empty(), "Should have generated an exit signal");

        let entry = entries[0];
        assert_eq!(entry.side, "buy");
        assert_eq!(entry.size_hint, "100");
        assert!(entry.stop_loss.is_some());
        assert!(entry.reason.contains("crossed above"));

        let exit = exits[0];
        assert_eq!(exit.side, "sell");
        assert_eq!(exit.size_hint, "max");
        assert!(exit.stop_loss.is_none());
        assert!(exit.reason.contains("crossed below"));

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let config = AdlMomentumConfig {
            adl_sma_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let mut strategy = AdlMomentum::new(config);

        let new_params = serde_json::json!({
            "adl_sma_period": 10,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 20,
            "symbol": "UPDATED"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.adl_sma_period, 10);
        assert_eq!(strategy.config.stop_loss_atr_mult, 1.5);
        assert_eq!(strategy.config.symbol, "UPDATED");

        let invalid_params = serde_json::json!({
            "adl_sma_period": 0,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 20,
            "symbol": "UPDATED"
        });
        assert!(strategy.update_params(invalid_params).await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let config = AdlMomentumConfig {
            adl_sma_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = AdlMomentum::new(config);

        let empty_df = DataFrame::empty();
        let result = strategy.generate_signals(&empty_df).await;
        assert!(result.is_err());

        Ok(())
    }
}
