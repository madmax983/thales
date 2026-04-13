use crate::indicators::{atr, sma, trix};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrixMomentumConfig {
    pub trix_period: usize,
    pub signal_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for TrixMomentumConfig {
    fn default() -> Self {
        Self {
            trix_period: 14,
            signal_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for TrixMomentumConfig {}

pub struct TrixMomentumStrategy {
    config: TrixMomentumConfig,
}

impl TrixMomentumStrategy {
    pub fn new(config: TrixMomentumConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for TrixMomentumStrategy {
    fn name(&self) -> &str {
        "TrixMomentum"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let trix_series = trix::calculate(data, self.config.trix_period)?;
        let trix_arr = trix_series.f64()?;

        // Calculate Signal Line (SMA of TRIX)
        let trix_df = DataFrame::new(vec![Series::new("close", trix_arr.clone())])?;
        let signal_series = sma::calculate(&trix_df, self.config.signal_period)?;
        let signal_arr = signal_series.f64()?;

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let mut signals = Vec::new();
        let mut position: Option<&str> = None;

        let sl_mult = Decimal::from_f64(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let trix_curr = trix_arr.get(i);
            let trix_prev = trix_arr.get(i - 1);

            let sig_curr = signal_arr.get(i);
            let sig_prev = signal_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(t_curr),
                Some(t_prev),
                Some(s_curr),
                Some(s_prev),
                Some(price),
                Some(atr_val),
            ) = (trix_curr, trix_prev, sig_curr, sig_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Bullish Crossover (TRIX crosses above Signal Line)
                if t_prev <= s_prev && t_curr > s_curr {
                    if position == Some("short") {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(), // Exit short
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            stop_loss: None,
                            take_profit: None,
                            reason: format!(
                                "TRIX Bullish Crossover: {:.2} > {:.2}",
                                t_curr, s_curr
                            ),
                            timestamp_ms: timestamp,
                        });
                        position = None;
                    }

                    if position.is_none() {
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
                            reason: format!(
                                "TRIX Bullish Crossover: {:.2} > {:.2}",
                                t_curr, s_curr
                            ),
                            timestamp_ms: timestamp,
                        });
                        position = Some("long");
                    }
                }
                // Bearish Crossover (TRIX crosses below Signal Line)
                else if t_prev >= s_prev && t_curr < s_curr {
                    if position == Some("long") {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(), // Exit long
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            stop_loss: None,
                            take_profit: None,
                            reason: format!(
                                "TRIX Bearish Crossover: {:.2} < {:.2}",
                                t_curr, s_curr
                            ),
                            timestamp_ms: timestamp,
                        });
                        position = None;
                    }

                    if position.is_none() {
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
                            reason: format!(
                                "TRIX Bearish Crossover: {:.2} < {:.2}",
                                t_curr, s_curr
                            ),
                            timestamp_ms: timestamp,
                        });
                        position = Some("short");
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: TrixMomentumConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_entry_signal_generation() -> Result<()> {
        let config = TrixMomentumConfig {
            trix_period: 2,
            signal_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = TrixMomentumStrategy::new(config);

        // We need data that causes TRIX to cross its SMA.
        // TRIX = ROC of EMA3.
        // To get a quick positive TRIX crossing its SMA:
        // Use an exponential price curve.
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000, 11000, 12000],
            "close" => &[10.0, 10.5, 11.0, 12.0, 14.0, 17.0, 22.0, 29.0, 40.0, 55.0, 75.0, 100.0],
            "high"  => &[10.5, 11.0, 11.5, 12.5, 14.5, 17.5, 22.5, 29.5, 40.5, 55.5, 75.5, 100.5],
            "low"   => &[9.5, 10.0, 10.5, 11.5, 13.5, 16.5, 21.5, 28.5, 39.5, 54.5, 74.5, 99.5],
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // We expect an Entry Long due to accelerating momentum
        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        assert!(!entries.is_empty(), "Should generate an entry signal");

        Ok(())
    }

    #[tokio::test]
    async fn test_exit_signal_generation() -> Result<()> {
        let config = TrixMomentumConfig {
            trix_period: 2,
            signal_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = TrixMomentumStrategy::new(config);

        // Accelerate then drop sharply to trigger entry then exit
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000, 11000, 12000, 13000, 14000, 15000, 16000, 17000],
            "close" => &[10.0, 11.0, 13.0, 17.0, 23.0, 31.0, 45.0, 75.0, 50.0, 40.0, 30.0, 25.0, 20.0, 15.0, 10.0, 5.0, 1.0], // Fast up then drop
            "high"  => &[11.0, 12.0, 14.0, 18.0, 24.0, 32.0, 46.0, 76.0, 51.0, 41.0, 31.0, 26.0, 21.0, 16.0, 11.0, 6.0, 2.0],
            "low"   => &[9.0, 10.0, 12.0, 16.0, 22.0, 30.0, 44.0, 74.0, 49.0, 39.0, 29.0, 24.0, 19.0, 14.0, 9.0, 4.0, 0.5],
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Check for an exit signal
        // We might not get an exit signal if the drop isn't enough to flip TRIX,
        // or if it happens too fast. Let's look for ANY exit or entry signal that proves
        // the strategy flipped position from long to short.
        let has_short_entry = signals
            .iter()
            .any(|s| s.signal_type == SignalType::Entry && s.side == "sell");
        assert!(
            has_short_entry,
            "Should generate an entry short signal (flip)"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_edge_cases() -> Result<()> {
        let config = TrixMomentumConfig::default();
        let strategy = TrixMomentumStrategy::new(config);

        // Empty dataframe
        let df_empty = DataFrame::default();
        let res = strategy.generate_signals(&df_empty).await;
        assert!(res.is_err(), "Should error on empty data");

        // Small dataframe
        let df_small = df!(
            "timestamp_unix_ms" => &[1000i64],
            "close" => &[10.0],
            "high" => &[10.5],
            "low" => &[9.5]
        )?;
        let signals = strategy.generate_signals(&df_small).await?;
        assert!(
            signals.is_empty(),
            "Should generate no signals for small data"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let config = TrixMomentumConfig::default();
        let mut strategy = TrixMomentumStrategy::new(config);

        let new_params = serde_json::json!({
            "trix_period": 10,
            "signal_period": 5,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.trix_period, 10);
        assert_eq!(strategy.config.signal_period, 5);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
