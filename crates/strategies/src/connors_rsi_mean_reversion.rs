//! The Connors RSI Mean Reversion Strategy
//!
//! A high-probability short-term mean reversion strategy using Connors RSI.
//!
use crate::indicators::{connors_rsi, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnorsRsiMeanReversionConfig {
    pub rsi_period: usize,
    pub streak_rsi_period: usize,
    pub rank_lookback: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_pct: f64,
    pub exit_sma_period: Option<usize>, // Optional Mean Reversion exit: Close > SMA
    pub symbol: String,
}

impl StrategyConfig for ConnorsRsiMeanReversionConfig {}

pub struct ConnorsRsiMeanReversion {
    config: ConnorsRsiMeanReversionConfig,
}

impl ConnorsRsiMeanReversion {
    pub fn new(config: ConnorsRsiMeanReversionConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for ConnorsRsiMeanReversion {
    fn name(&self) -> &str {
        "ConnorsRsiMeanReversion"
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

        // Calculate Connors RSI
        let crsi_series = connors_rsi::calculate(
            data,
            self.config.rsi_period,
            self.config.streak_rsi_period,
            self.config.rank_lookback,
        )?;
        let crsi_arr = crsi_series.f64()?;

        // Calculate Optional SMA for Exit
        let sma_arr = if let Some(period) = self.config.exit_sma_period {
            let s = sma::calculate(data, period)?;
            Some(s.f64()?.clone().into_iter().collect::<Vec<_>>())
        } else {
            None
        };

        let mut signals = Vec::new();
        let stop_loss_pct_dec =
            Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);
            let crsi_opt = crsi_arr.get(i);

            if let (Some(price), Some(crsi_val)) = (price_opt, crsi_opt) {
                // Check for Exit
                // 1. Overbought Threshold
                // 2. Or Price > SMA (if configured)
                let mut exit_signal = false;
                let mut exit_reason = String::new();

                if crsi_val > self.config.overbought_threshold {
                    exit_signal = true;
                    exit_reason = format!(
                        "CRSI Overbought: {:.2} > {:.2}",
                        crsi_val, self.config.overbought_threshold
                    );
                } else if let Some(ref sma_vals) = sma_arr {
                    if let Some(Some(sma_val)) = sma_vals.get(i) {
                        if price.to_f64().unwrap_or(0.0) > *sma_val {
                            exit_signal = true;
                            exit_reason = format!(
                                "Price {:.2} > SMA({}) {:.2}",
                                price,
                                self.config.exit_sma_period.unwrap(),
                                sma_val
                            );
                        }
                    }
                }

                if exit_signal {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: exit_reason,
                        timestamp_ms: timestamp,
                    });
                }

                // Check for Entry (Oversold)
                if crsi_val < self.config.oversold_threshold {
                    let sl = price * (one_dec - stop_loss_pct_dec);
                    // TP: Mean Reversion usually targets the SMA or overbought.
                    // We can set a heuristic TP or rely on Exit signal.
                    // Let's set TP at 5% above for now or rely on dynamic management.
                    let tp = price * (Decimal::new(105, 2));

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "CRSI Oversold: {:.2} < {:.2}",
                            crsi_val, self.config.oversold_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: ConnorsRsiMeanReversionConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_connors_rsi_signals() -> Result<()> {
        let config = ConnorsRsiMeanReversionConfig {
            rsi_period: 2,
            streak_rsi_period: 2,
            rank_lookback: 5, // Short lookback for test
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            stop_loss_pct: 0.05,
            exit_sma_period: None,
            symbol: "TEST".to_string(),
        };
        let strategy = ConnorsRsiMeanReversion::new(config);

        // Construct data
        // Need enough data for rank lookback (5) + rsi (2) etc.
        // Let's create a drop to trigger Oversold.

        let mut closes = vec![100.0; 10];
        // Drop: 100, 99, 98, 95, 90 (Streak down, RSI low, Rank low)
        closes.extend_from_slice(&[99.0, 98.0, 95.0, 90.0]);
        // Rally: 95, 100, 110 (Streak up, RSI high, Rank high)
        closes.extend_from_slice(&[95.0, 100.0, 110.0]);

        let timestamps: Vec<i64> = (0..closes.len()).map(|i| 1000 + i as i64 * 1000).collect();

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect Entry near index 13/14 (90.0) where RSI is low
        // Expect Exit near index 16 (110.0) where RSI is high

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        // Debug prints if needed
        // for s in &signals { println!("{:?}", s); }

        assert!(!entries.is_empty(), "Should generate entry signal");
        assert!(!exits.is_empty(), "Should generate exit signal");

        // Verify reasons
        assert!(entries[0].reason.contains("CRSI Oversold"));
        assert!(exits[0].reason.contains("CRSI Overbought"));

        Ok(())
    }

    #[tokio::test]
    async fn test_sma_exit() -> Result<()> {
        let config = ConnorsRsiMeanReversionConfig {
            rsi_period: 2,
            streak_rsi_period: 2,
            rank_lookback: 5,
            oversold_threshold: 10.0,
            overbought_threshold: 101.0, // Set > 100 to ensure CRSI doesn't trigger, testing SMA logic
            stop_loss_pct: 0.05,
            exit_sma_period: Some(3),
            symbol: "TEST".to_string(),
        };
        let strategy = ConnorsRsiMeanReversion::new(config);

        // Data:
        // Flat 100. SMA(3) = 100.
        // Jump to 105. Price > SMA(3) -> Exit.

        let mut closes = vec![100.0; 10];
        closes.push(105.0);

        let timestamps: Vec<i64> = (0..closes.len()).map(|i| 1000 + i as i64 * 1000).collect();

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        assert!(!exits.is_empty());
        let exit = exits.last().unwrap();
        assert!(exit.reason.contains("Price 105.00 > SMA(3)"));

        Ok(())
    }
}
