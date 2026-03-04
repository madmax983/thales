use crate::indicators::{atr, stoch_rsi};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StochRsiMeanReversionConfig {
    pub rsi_period: usize,
    pub stoch_period: usize,
    pub k_period: usize,
    pub d_period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for StochRsiMeanReversionConfig {}

pub struct StochRsiMeanReversion {
    config: StochRsiMeanReversionConfig,
}

impl StochRsiMeanReversion {
    pub fn new(config: StochRsiMeanReversionConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for StochRsiMeanReversion {
    fn name(&self) -> &str {
        "StochRsiMeanReversion"
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

        // Calculate StochRSI
        let (k_series, d_series) = stoch_rsi::calculate(
            data,
            self.config.rsi_period,
            self.config.stoch_period,
            self.config.k_period,
            self.config.d_period,
        )?;
        let k_arr = k_series.f64()?;
        let d_arr = d_series.f64()?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let k_curr = k_arr.get(i);
            let k_prev = k_arr.get(i - 1);
            let d_curr = d_arr.get(i);
            let d_prev = d_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (Some(k_c), Some(k_p), Some(d_c), Some(d_p), Some(price), Some(atr_val)) =
                (k_curr, k_prev, d_curr, d_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Long Entry: %K crosses above %D AND %K is in oversold region (< threshold)
                if k_p <= d_p && k_c > d_c && k_c < self.config.oversold_threshold {
                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short (Buy to cover)
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "StochRSI Bullish Crossover: %K {:.2} > %D {:.2} (Oversold < {})",
                            k_c, d_c, self.config.oversold_threshold
                        ),
                        timestamp_ms: timestamp,
                    });

                    // Enter Long
                    let sl = price_dec - (atr_dec * sl_mult);
                    let risk = price_dec - sl;
                    // Example TP based on 2:1 risk reward
                    let two_dec = Decimal::from(2);
                    let tp = price_dec + (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "StochRSI Long Entry: %K {:.2} > %D {:.2} (Oversold < {})",
                            k_c, d_c, self.config.oversold_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Short Entry: %K crosses below %D AND %K is in overbought region (> threshold)
                else if k_p >= d_p && k_c < d_c && k_c > self.config.overbought_threshold {
                    // Exit any Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "StochRSI Bearish Crossover: %K {:.2} < %D {:.2} (Overbought > {})",
                            k_c, d_c, self.config.overbought_threshold
                        ),
                        timestamp_ms: timestamp,
                    });

                    // Enter Short
                    let sl = price_dec + (atr_dec * sl_mult);
                    let risk = sl - price_dec;
                    let two_dec = Decimal::from(2);
                    let tp = price_dec - (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "StochRSI Short Entry: %K {:.2} < %D {:.2} (Overbought > {})",
                            k_c, d_c, self.config.overbought_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: StochRsiMeanReversionConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stoch_rsi_signals() -> Result<()> {
        let config = StochRsiMeanReversionConfig {
            rsi_period: 3,
            stoch_period: 3,
            k_period: 3,
            d_period: 3,
            oversold_threshold: 40.0,
            overbought_threshold: 60.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = StochRsiMeanReversion::new(config);

        // In order to trigger a long entry, we need %K to be < 20 and cross above %D.
        // We'll create a DataFrame with enough points to establish a strong trend down (RSI near 0, StochRSI near 0),
        // followed by a sudden reversal up.
        // Let's just generate a large enough trend down then up.
        let values: Vec<f64> = vec![
            100.0, 95.0, 90.0, 85.0, 80.0, 75.0, 70.0, 65.0, 60.0, 55.0, 50.0, 45.0, 40.0, 35.0,
            30.0, 31.0, 32.0, 35.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0, 110.0, 120.0,
        ];
        let timestamps: Vec<i64> = (0..values.len()).map(|i| i as i64 * 1000).collect();

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "open"  => values.clone(),
            "high"  => values.clone(),
            "low"   => values.clone(),
            "close" => values.clone()
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // We expect at least one buy signal as the price reverses from 30 up to 120.
        let buy_entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();

        assert!(
            !buy_entries.is_empty(),
            "Expected a buy signal during the reversal up."
        );

        // And we expect at least one sell signal as the price reaches the top (overbought) and maybe flattens or drops.
        // Wait, price keeps going up in my vector, so StochRSI will stay at 100.
        // We need it to cross down for a sell.
        // Let's add a drop at the end of the data.
        let values2: Vec<f64> = vec![
            100.0, 95.0, 90.0, 85.0, 80.0, 75.0, 70.0, 65.0, 60.0, 55.0, 50.0, 45.0, 40.0, 35.0,
            30.0, 31.0, 32.0, 35.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0, 110.0, 120.0, 115.0,
            110.0, 105.0, 100.0,
        ];
        let timestamps2: Vec<i64> = (0..values2.len()).map(|i| i as i64 * 1000).collect();

        let df2 = df!(
            "timestamp_unix_ms" => timestamps2,
            "open"  => values2.clone(),
            "high"  => values2.clone(),
            "low"   => values2.clone(),
            "close" => values2.clone()
        )?;

        let signals2 = strategy.generate_signals(&df2).await?;
        let sell_entries: Vec<_> = signals2
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "sell")
            .collect();

        assert!(
            !sell_entries.is_empty(),
            "Expected a sell signal during the reversal down."
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = StochRsiMeanReversion::new(StochRsiMeanReversionConfig {
            rsi_period: 14,
            stoch_period: 14,
            k_period: 3,
            d_period: 3,
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "rsi_period": 10,
            "stoch_period": 10,
            "k_period": 5,
            "d_period": 5,
            "oversold_threshold": 30.0,
            "overbought_threshold": 70.0,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "NEW_TEST"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.rsi_period, 10);
        assert_eq!(strategy.config.stoch_period, 10);
        assert_eq!(strategy.config.k_period, 5);
        assert_eq!(strategy.config.d_period, 5);
        assert_eq!(strategy.config.oversold_threshold, 30.0);
        assert_eq!(strategy.config.overbought_threshold, 70.0);
        assert_eq!(strategy.config.symbol, "NEW_TEST");

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let config = StochRsiMeanReversionConfig {
            rsi_period: 14,
            stoch_period: 14,
            k_period: 3,
            d_period: 3,
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = StochRsiMeanReversion::new(config);

        let df_empty = DataFrame::default();
        let res = strategy.generate_signals(&df_empty).await;

        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("not found"));

        Ok(())
    }
}
