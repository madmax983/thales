use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig};
use crate::indicators::{atr, donchian_channels};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonchianBreakoutConfig {
    pub entry_period: usize,
    pub exit_period: usize,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}

impl StrategyConfig for DonchianBreakoutConfig {}

pub struct DonchianBreakout {
    config: DonchianBreakoutConfig,
}

impl DonchianBreakout {
    pub fn new(config: DonchianBreakoutConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for DonchianBreakout {
    fn name(&self) -> &str {
        "DonchianBreakout"
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        // Validate inputs - need at least min(entry, exit) + 1 to have one shifted value
        let min_period = std::cmp::min(self.config.entry_period, self.config.exit_period);
        if data.height() < min_period + 1 {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate Channels using new indicator
        // Entry Channel (Upper) uses entry_period
        let (_, _, upper_series) = donchian_channels::calculate(data, self.config.entry_period)?;
        // Exit Channel (Lower) uses exit_period
        let (lower_series, _, _) = donchian_channels::calculate(data, self.config.exit_period)?;

        // Convert to Vec<Option<f64>> for easy access
        let upper_channel_vec: Vec<Option<f64>> = upper_series.f64()?.into_iter().map(|v| v).collect();
        let lower_channel_vec: Vec<Option<f64>> = lower_series.f64()?.into_iter().map(|v| v).collect();

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, 14)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_mult = Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let len = data.height();

        // Iterate through data
        for i in min_period..len {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_series.get(i);

            // Shifted check: We need channel value from i-1
            let upper_val_prev = if i > 0 { upper_channel_vec.get(i-1).copied().flatten() } else { None };
            let lower_val_prev = if i > 0 { lower_channel_vec.get(i-1).copied().flatten() } else { None };
            let atr_opt = atr_arr.get(i);

            if let Some(price) = price_opt {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);

                // Entry Condition: Close > Upper Channel (from previous bar)
                if let Some(upper) = upper_val_prev {
                    if price > upper {
                         let sl = if let Some(atr_val) = atr_opt {
                            let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);
                            Some((price_dec - (atr_dec * stop_loss_mult)).to_f64().unwrap_or(0.0))
                        } else {
                            // Fallback stop loss if ATR is not available
                            // Use Lower Channel as strict stop if available
                             lower_val_prev.or(Some(upper * 0.95)) // 5% fallback if no lower channel
                        };

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: sl,
                            take_profit: None,
                            reason: format!("Breakout: Close {:.2} > Upper Channel {:.2}", price, upper),
                            timestamp_ms: timestamp,
                        });
                    }
                }

                // Exit Condition: Close < Lower Channel (from previous bar)
                if let Some(lower) = lower_val_prev {
                    if price < lower {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            stop_loss: None,
                            take_profit: None,
                            reason: format!("Breakdown: Close {:.2} < Lower Channel {:.2}", price, lower),
                            timestamp_ms: timestamp,
                        });
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: DonchianBreakoutConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_donchian_entry() -> Result<()> {
        let config = DonchianBreakoutConfig {
            entry_period: 3,
            exit_period: 2,
            stop_loss_atr_mult: 1.0,
            symbol: "TEST".to_string(),
        };
        let strategy = DonchianBreakout::new(config);

        // Data:
        // 0: High 100
        // 1: High 102
        // 2: High 101
        // Max(3) at index 2 (covering 0,1,2) = 102.
        // We use shifted value at index 3 -> prev val at index 2 -> 102.
        // 3: Close 103 -> Breakout > 102 -> Entry.

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "high" => &[100.0, 102.0, 101.0, 104.0],
            "low" => &[90.0, 92.0, 91.0, 94.0],
            "close" => &[95.0, 96.0, 95.0, 103.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let entries: Vec<_> = signals.iter().filter(|s| s.signal_type == SignalType::Entry).collect();
        assert_eq!(entries.len(), 1);
        let entry = entries[0];
        assert_eq!(entry.timestamp_ms, 4000);
        assert!(entry.reason.contains("Breakout"));

        Ok(())
    }

    #[tokio::test]
    async fn test_donchian_exit() -> Result<()> {
        let config = DonchianBreakoutConfig {
            entry_period: 3,
            exit_period: 2,
            stop_loss_atr_mult: 1.0,
            symbol: "TEST".to_string(),
        };
        let strategy = DonchianBreakout::new(config);

        // Data:
        // 0: Low 100
        // 1: Low 102
        // Min(2) at index 1 (covering 0,1) = 100.
        // We use shifted value at index 2 -> prev val at index 1 -> 100.
        // 2: Close 99 < 100 -> Exit.

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000],
            "high" => &[110.0, 112.0, 111.0],
            "low" => &[100.0, 102.0, 98.0],
            "close" => &[105.0, 106.0, 99.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let exits: Vec<_> = signals.iter().filter(|s| s.signal_type == SignalType::Exit).collect();
        assert_eq!(exits.len(), 1);
        let exit = exits[0];
        assert_eq!(exit.timestamp_ms, 3000);
        assert!(exit.reason.contains("Breakdown"));

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let config = DonchianBreakoutConfig {
            entry_period: 10,
            exit_period: 5,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = DonchianBreakout::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000],
            "high" => &[100.0, 102.0],
            "low" => &[90.0, 92.0],
            "close" => &[95.0, 96.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_mixed_availability() -> Result<()> {
        // Test where exit is available but entry is not
        let config = DonchianBreakoutConfig {
            entry_period: 10,
            exit_period: 2,
            stop_loss_atr_mult: 1.0,
            symbol: "TEST".to_string(),
        };
        let strategy = DonchianBreakout::new(config);

        // 3 bars.
        // Exit needs 2 bars. Min(2) available at index 1. Shifted -> index 2.
        // Entry needs 10 bars. Not available.
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000],
            "high" => &[100.0, 100.0, 100.0],
            "low" => &[90.0, 92.0, 80.0],
            // Index 0: Low 90
            // Index 1: Low 92. RollingMin(2) = 90.
            // Index 2: RollingMin shifted from index 1 is 90.
            // Close 85 < 90 -> Exit.
            "close" => &[95.0, 96.0, 85.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let exits: Vec<_> = signals.iter().filter(|s| s.signal_type == SignalType::Exit).collect();
        assert_eq!(exits.len(), 1);

        let entries: Vec<_> = signals.iter().filter(|s| s.signal_type == SignalType::Entry).collect();
        assert_eq!(entries.len(), 0);

        Ok(())
    }
}
