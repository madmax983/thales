use crate::indicators::{atr, eom, sma};
use crate::strategy::{Signal, SignalType, Strategy};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Configuration for the Ease of Movement Trend Strategy.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EomTrendConfig {
    /// The period for calculating the Ease of Movement indicator.
    pub eom_period: usize,
    /// The period for the SMA of the EOM indicator, which acts as the signal line.
    pub sma_period: usize,
    /// The multiplier for the ATR to calculate the stop loss.
    pub stop_loss_atr_mult: f64,
    /// The period for calculating the ATR.
    pub atr_period: usize,
    /// The trading pair or asset symbol.
    pub symbol: String,
}

impl Default for EomTrendConfig {
    fn default() -> Self {
        Self {
            eom_period: 14,
            sma_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

/// A trend-following strategy based on the Ease of Movement (EOM) indicator.
pub struct EomTrend {
    config: EomTrendConfig,
}

impl EomTrend {
    /// Creates a new EomTrend strategy with the given configuration.
    pub fn new(config: EomTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for EomTrend {
    fn name(&self) -> &str {
        "EomTrend"
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        // Validation of data length
        let min_len = self.config.eom_period + self.config.sma_period + self.config.atr_period;
        if data.height() < min_len {
            return Ok(vec![]);
        }

        // Calculate EOM
        let mut eom_series = eom::calculate(data, self.config.eom_period)?;
        eom_series.rename("close");
        let eom_df = DataFrame::new(vec![eom_series.clone()])?;

        // Calculate Signal Line (SMA of EOM)
        let mut signal_series = sma::calculate(&eom_df, self.config.sma_period)?;
        signal_series.rename("signal");

        // Calculate ATR for stop loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let eom_vals = eom_series.f64()?;
        let signal_vals = signal_series.f64()?;
        let atr_vals = atr_series.f64()?;
        let close_col = data.column("close")?.f64()?;

        let mut timestamp_col: Option<&ChunkedArray<Int64Type>> = None;
        if let Ok(ts_col) = data.column("timestamp_unix_ms") {
            // Polars often treats Int64 missing values as Int64 directly
            if let Ok(ts_i64) = ts_col.i64() {
                timestamp_col = Some(ts_i64);
            } else if let Ok(ts_u64) = ts_col.u64() {
                // If it was cast or provided as u64, we'd need to handle it or cast it
                // We'll rely on try_cast below if needed.
            }
        }

        let ts_col_cast = data.column("timestamp_unix_ms").and_then(|c| c.cast(&DataType::Int64)).ok();
        if let Some(ref col) = ts_col_cast {
            timestamp_col = col.i64().ok();
        }

        let mut signals = Vec::new();

        let len = data.height();
        for i in 1..len {
            if let (
                Some(prev_eom),
                Some(prev_signal),
                Some(curr_eom),
                Some(curr_signal),
                Some(close),
                Some(atr_val),
            ) = (
                eom_vals.get(i - 1),
                signal_vals.get(i - 1),
                eom_vals.get(i),
                signal_vals.get(i),
                close_col.get(i),
                atr_vals.get(i),
            ) {
                let ts = timestamp_col.and_then(|c| c.get(i)).unwrap_or(0);

                // Check for Entry Long / Exit Short: EOM crosses ABOVE Signal Line
                if prev_eom <= prev_signal && curr_eom > curr_signal {
                    // Close any open short positions
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.85,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "EOM ({:.2}) crossed above Signal Line ({:.2})",
                            curr_eom, curr_signal
                        ),
                        timestamp_ms: ts,
                    });

                    // Only generate entry if we can safely calculate SL
                    let sl_opt = Decimal::from_f64_retain(close).and_then(|c| {
                        Decimal::from_f64_retain(atr_val).and_then(|a| {
                            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).map(|m| c - (a * m))
                        })
                    });

                    if let Some(sl) = sl_opt {
                        // Open a new long position
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.85,
                            stop_loss: sl.to_f64(),
                            take_profit: None,
                            reason: format!(
                                "EOM ({:.2}) crossed above Signal Line ({:.2})",
                                curr_eom, curr_signal
                            ),
                            timestamp_ms: ts,
                        });
                    }
                }
                // Check for Entry Short / Exit Long: EOM crosses BELOW Signal Line
                else if prev_eom >= prev_signal && curr_eom < curr_signal {
                    // Close any open long positions
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.85,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "EOM ({:.2}) crossed below Signal Line ({:.2})",
                            curr_eom, curr_signal
                        ),
                        timestamp_ms: ts,
                    });

                    // Only generate entry if we can safely calculate SL
                    let sl_opt = Decimal::from_f64_retain(close).and_then(|c| {
                        Decimal::from_f64_retain(atr_val).and_then(|a| {
                            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).map(|m| c + (a * m))
                        })
                    });

                    if let Some(sl) = sl_opt {
                        // Open a new short position
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.85,
                            stop_loss: sl.to_f64(),
                            take_profit: None,
                            reason: format!(
                                "EOM ({:.2}) crossed below Signal Line ({:.2})",
                                curr_eom, curr_signal
                            ),
                            timestamp_ms: ts,
                        });
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: EomTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_eom_trend_signal_generation() -> Result<()> {
        let config = EomTrendConfig {
            eom_period: 2,
            sma_period: 2,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = EomTrend::new(config);

        // Need enough data
        let high = vec![10.0, 11.0, 12.0, 13.0, 14.0, 13.0, 12.0, 11.0];
        let low = vec![9.0, 10.0, 11.0, 12.0, 13.0, 12.0, 11.0, 10.0];
        let close = vec![9.5, 10.5, 11.5, 12.5, 13.5, 12.5, 11.5, 10.5];
        let volume = vec![
            100_000_000.0,
            200_000_000.0,
            150_000_000.0,
            250_000_000.0,
            100_000_000.0,
            50_000_000.0,
            200_000_000.0,
            100_000_000.0,
        ];
        let timestamp = vec![100i64, 200i64, 300i64, 400i64, 500i64, 600i64, 700i64, 800i64];

        let df = df!(
            "high" => &high,
            "low" => &low,
            "close" => &close,
            "volume" => &volume,
            "timestamp_unix_ms" => &timestamp
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect signals to be generated
        assert!(!signals.is_empty());

        // Assert we have Exit and Entry pairs
        let exits: Vec<_> = signals.iter().filter(|s| s.signal_type == SignalType::Exit).collect();
        let entries: Vec<_> = signals.iter().filter(|s| s.signal_type == SignalType::Entry).collect();

        assert!(!exits.is_empty());
        assert!(!entries.is_empty());

        // Check fields logic
        for s in &signals {
            assert_eq!(s.symbol, "TEST");
            assert!(s.timestamp_ms > 0);
            if s.signal_type == SignalType::Exit {
                assert_eq!(s.size_hint, "max");
            } else {
                assert_eq!(s.size_hint, "100");
                assert!(s.stop_loss.is_some());
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_update() -> Result<()> {
        let mut strategy = EomTrend::new(EomTrendConfig::default());
        let new_params = serde_json::json!({
            "eom_period": 5,
            "sma_period": 3,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.eom_period, 5);
        assert_eq!(strategy.config.sma_period, 3);
        assert_eq!(strategy.config.symbol, "BTCUSD");
        Ok(())
    }

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let strategy = EomTrend::new(EomTrendConfig::default());
        let df = DataFrame::default();
        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_insufficient_data() -> Result<()> {
        let strategy = EomTrend::new(EomTrendConfig::default()); // Needs at least 14 + 9 + 14 = 37 rows
        let df = df!(
            "high" => &[10.0, 11.0],
            "low" => &[9.0, 10.0],
            "close" => &[9.5, 10.5],
            "volume" => &[1000.0, 2000.0]
        )?;
        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());
        Ok(())
    }
}
