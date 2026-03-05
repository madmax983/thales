use crate::indicators::{atr, sma, trix};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

/// Configuration for the TRIX Momentum strategy.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TrixMomentumConfig {
    pub trix_period: usize,
    pub signal_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for TrixMomentumConfig {}

/// TRIX Momentum Strategy
///
/// A momentum strategy that uses the TRIX indicator and its Signal Line (SMA of TRIX)
/// to identify trend direction and momentum shifts.
pub struct TrixMomentum {
    config: TrixMomentumConfig,
}

impl TrixMomentum {
    pub fn new(config: TrixMomentumConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for TrixMomentum {
    fn name(&self) -> &str {
        "TrixMomentum"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height()
            < self.config.trix_period * 3 + self.config.signal_period + self.config.atr_period
        {
            return Ok(vec![]);
        }

        // Calculate TRIX
        let mut trix_series = trix::calculate(data, self.config.trix_period)?;
        let trix_df = DataFrame::new(vec![trix_series.rename("close".into()).clone()])?;

        // Calculate Signal Line (SMA of TRIX)
        let signal_series = sma::calculate(&trix_df, self.config.signal_period)?;

        // Calculate ATR for stop loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let trix_f64 = trix_series.f64()?;
        let signal_f64 = signal_series.f64()?;
        let atr_f64 = atr_series.f64()?;

        let close_col = data.column("close")?.f64()?;
        let time_col = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let mut in_long = false;
        let mut _entry_price = 0.0;
        let mut stop_loss = 0.0;

        for i in 1..data.height() {
            let curr_trix = trix_f64.get(i);
            let prev_trix = trix_f64.get(i - 1);
            let curr_signal = signal_f64.get(i);
            let prev_signal = signal_f64.get(i - 1);

            let close = close_col.get(i);
            let atr_val = atr_f64.get(i);
            let timestamp = time_col.get(i).unwrap_or(0);

            if let (Some(c_trix), Some(p_trix), Some(c_sig), Some(p_sig), Some(price), Some(atr)) = (
                curr_trix,
                prev_trix,
                curr_signal,
                prev_signal,
                close,
                atr_val,
            ) {
                // Check stop loss first
                if in_long && price <= stop_loss {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 1.0,
                        stop_loss: None,
                        take_profit: None,
                        reason: "TRIX Momentum Stop Loss Hit".to_string(),
                        timestamp_ms: timestamp,
                    });
                    in_long = false;
                    continue;
                }

                // TRIX crosses ABOVE Signal Line -> Buy
                if !in_long && p_trix <= p_sig && c_trix > c_sig {
                    let calculated_sl = price - (atr * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(calculated_sl),
                        take_profit: None,
                        reason: "TRIX crosses above Signal Line".to_string(),
                        timestamp_ms: timestamp,
                    });
                    in_long = true;
                    _entry_price = price;
                    stop_loss = calculated_sl;
                }
                // TRIX crosses BELOW Signal Line -> Sell
                else if in_long && p_trix >= p_sig && c_trix < c_sig {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "TRIX crosses below Signal Line".to_string(),
                        timestamp_ms: timestamp,
                    });
                    in_long = false;
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

    fn create_mock_data(size: usize) -> DataFrame {
        // Create an oscillating price to generate crossovers
        let close: Vec<f64> = (0..size)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let high: Vec<f64> = close.iter().map(|c| c + 1.0).collect();
        let low: Vec<f64> = close.iter().map(|c| c - 1.0).collect();
        let open: Vec<f64> = close.clone();
        let volume: Vec<f64> = vec![1000.0; size];
        let timestamp: Vec<i64> = (0..size).map(|i| i as i64 * 60000).collect();

        df!(
            "open" => open,
            "high" => high,
            "low" => low,
            "close" => close,
            "volume" => volume,
            "timestamp_unix_ms" => timestamp
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let strategy = TrixMomentum::new(TrixMomentumConfig {
            trix_period: 15,
            signal_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });
        let df = DataFrame::default();
        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_signal_generation() -> Result<()> {
        let strategy = TrixMomentum::new(TrixMomentumConfig {
            trix_period: 5, // smaller periods so we get valid data sooner
            signal_period: 3,
            stop_loss_atr_mult: 2.0,
            atr_period: 5,
            symbol: "TEST".to_string(),
        });

        // 5 * 3 (TRIX triple EMA) + 3 (SMA) + 5 (ATR) = 15 + 3 + 5 = 23.
        // Needs at least 23 rows. Let's provide 100 rows.
        let df = create_mock_data(100);
        let signals = strategy.generate_signals(&df).await?;

        // With an oscillating sine wave, we should get multiple crossovers -> entry and exit pairs.
        assert!(!signals.is_empty(), "Should generate some signals");

        let mut in_position = false;
        for sig in signals {
            if sig.signal_type == SignalType::Entry {
                assert!(!in_position);
                assert_eq!(sig.side, "buy");
                assert!(sig.stop_loss.is_some());
                in_position = true;
            } else if sig.signal_type == SignalType::Exit {
                assert!(in_position);
                assert_eq!(sig.side, "sell");
                in_position = false;
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let mut strategy = TrixMomentum::new(TrixMomentumConfig {
            trix_period: 15,
            signal_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "trix_period": 20,
            "signal_period": 10,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 20,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.trix_period, 20);
        assert_eq!(strategy.config.signal_period, 10);
        assert_eq!(strategy.config.symbol, "BTCUSD");
        assert_eq!(strategy.config.atr_period, 20);

        Ok(())
    }
}
