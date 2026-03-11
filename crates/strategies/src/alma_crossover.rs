use crate::indicators::{alma, atr};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlmaCrossoverConfig {
    pub fast_period: usize,
    pub slow_period: usize,
    pub offset: f64,
    pub sigma: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for AlmaCrossoverConfig {}

pub struct AlmaCrossover {
    config: AlmaCrossoverConfig,
}

impl AlmaCrossover {
    pub fn new(config: AlmaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for AlmaCrossover {
    fn name(&self) -> &str {
        "AlmaCrossover"
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

        // Calculate ALMAs
        let fast_alma_series = alma::calculate(
            data,
            self.config.fast_period,
            self.config.offset,
            self.config.sigma,
        )?;
        let fast_alma_arr = fast_alma_series.f64()?;

        let slow_alma_series = alma::calculate(
            data,
            self.config.slow_period,
            self.config.offset,
            self.config.sigma,
        )?;
        let slow_alma_arr = slow_alma_series.f64()?;

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

            let fast_curr_opt = fast_alma_arr.get(i);
            let fast_prev_opt = fast_alma_arr.get(i - 1);
            let slow_curr_opt = slow_alma_arr.get(i);
            let slow_prev_opt = slow_alma_arr.get(i - 1);

            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (
                Some(price),
                Some(fast_curr),
                Some(fast_prev),
                Some(slow_curr),
                Some(slow_prev),
            ) = (
                price_opt,
                fast_curr_opt,
                fast_prev_opt,
                slow_curr_opt,
                slow_prev_opt,
            ) {
                // Long Entry (Buy): Fast ALMA crosses ABOVE Slow ALMA
                if fast_prev <= slow_prev && fast_curr > slow_curr {
                    let sl = if let Some(atr_val) = atr_opt {
                        price - (atr_val * atr_mult_dec)
                    } else {
                        // Fallback stop loss if ATR is missing (e.g. 2%)
                        price * Decimal::from_f64_retain(0.98).unwrap_or(Decimal::ONE)
                    };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Enter Long
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None, // Follow trend
                        reason: format!(
                            "Fast ALMA ({:.2}) crossed above Slow ALMA ({:.2})",
                            fast_curr, slow_curr
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Long Exit (Sell): Fast ALMA crosses BELOW Slow ALMA
                else if fast_prev >= slow_prev && fast_curr < slow_curr {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Fast ALMA ({:.2}) crossed below Slow ALMA ({:.2})",
                            fast_curr, slow_curr
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: AlmaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_df() -> Result<DataFrame> {
        let timestamps: Vec<i64> = (1..=20).map(|i| i * 1000).collect();
        // create a series where fast crosses slow and then crosses below
        // we can use a sharp price change for fast crossover
        // initially price is flat, fast and slow are flat
        // then price shoots up (fast crosses above slow)
        // then price shoots down (fast crosses below slow)
        let closes: Vec<f64> = vec![
            10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, // indices 0-9
            20.0, 20.0, 20.0, 20.0, 20.0, // indices 10-14, jump up to cause fast > slow
            5.0, 5.0, 5.0, 5.0, 5.0, // indices 15-19, jump down to cause fast < slow
        ];

        // Highs and Lows for ATR
        let highs: Vec<f64> = closes.iter().map(|&c| c + 1.0).collect();
        let lows: Vec<f64> = closes.iter().map(|&c| c - 1.0).collect();

        Ok(df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?)
    }

    #[tokio::test]
    async fn test_crossover_signals() -> Result<()> {
        let config = AlmaCrossoverConfig {
            fast_period: 2,
            slow_period: 5,
            offset: 0.85,
            sigma: 6.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = AlmaCrossover::new(config);
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

        // Check that we got at least one entry and one exit
        assert!(!entries.is_empty(), "Should have generated an entry signal");
        assert!(!exits.is_empty(), "Should have generated an exit signal");

        // Validate the first entry
        let entry = entries[0];
        assert_eq!(entry.side, "buy");
        assert_eq!(entry.size_hint, "100");
        assert!(entry.stop_loss.is_some());
        assert!(entry.reason.contains("crossed above"));

        // Validate the first exit
        let exit = exits[0];
        assert_eq!(exit.side, "sell");
        assert_eq!(exit.size_hint, "max");
        assert!(exit.stop_loss.is_none());
        assert!(exit.reason.contains("crossed below"));

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let config = AlmaCrossoverConfig {
            fast_period: 9,
            slow_period: 21,
            offset: 0.85,
            sigma: 6.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let mut strategy = AlmaCrossover::new(config);

        let new_params = serde_json::json!({
            "fast_period": 10,
            "slow_period": 30,
            "offset": 0.85,
            "sigma": 6.0,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 20,
            "symbol": "UPDATED"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.fast_period, 10);
        assert_eq!(strategy.config.slow_period, 30);
        assert_eq!(strategy.config.stop_loss_atr_mult, 1.5);
        assert_eq!(strategy.config.symbol, "UPDATED");

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let config = AlmaCrossoverConfig {
            fast_period: 9,
            slow_period: 21,
            offset: 0.85,
            sigma: 6.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = AlmaCrossover::new(config);

        let empty_df = DataFrame::empty();
        let result = strategy.generate_signals(&empty_df).await;
        assert!(
            result.is_err(),
            "Empty dataframe should yield an error due to missing columns/data"
        );

        Ok(())
    }
}
