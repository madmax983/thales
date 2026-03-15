use crate::indicators::{atr, ema, force_index};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceIndexTrendConfig {
    pub fi_period: usize,
    pub fi_ema_period: usize, // Optional: smooth FI itself, standard is usually 13 EMA
    pub price_ema_period: usize, // e.g. 22
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl ForceIndexTrendConfig {
    pub fn validate(&self) -> Result<()> {
        if self.fi_period == 0 {
            anyhow::bail!("fi_period must be > 0");
        }
        if self.price_ema_period == 0 {
            anyhow::bail!("price_ema_period must be > 0");
        }
        if self.atr_period == 0 {
            anyhow::bail!("atr_period must be > 0");
        }
        Ok(())
    }
}

impl StrategyConfig for ForceIndexTrendConfig {}

pub struct ForceIndexTrend {
    config: ForceIndexTrendConfig,
}

impl ForceIndexTrend {
    pub fn new(config: ForceIndexTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for ForceIndexTrend {
    fn name(&self) -> &str {
        "ForceIndexTrend"
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

        // Calculate Force Index (typically smoothed with EMA)
        let fi_series = force_index::calculate(data, self.config.fi_period)?;
        let fi_arr = fi_series.f64()?;

        // Optional smoothing for the signal
        let fi_df = DataFrame::new(vec![Series::new("close", fi_arr.clone())])?;
        let fi_ema_series = ema::calculate(&fi_df, self.config.fi_ema_period)?;
        let fi_ema_arr = fi_ema_series.f64()?;

        // Calculate Price EMA for trend direction
        let price_ema_series = ema::calculate(data, self.config.price_ema_period)?;
        let price_ema_arr = price_ema_series.f64()?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let fi_ema_curr = fi_ema_arr.get(i);
            let fi_ema_prev = fi_ema_arr.get(i - 1);
            let _fi_curr = fi_arr.get(i);

            let price_curr = close_arr.get(i);
            let price_ema_curr_val = price_ema_arr.get(i);

            let atr_opt = atr_arr.get(i);

            if let (
                Some(fi_ema_c),
                Some(fi_ema_p),
                Some(price),
                Some(price_ema_c),
                Some(atr_val),
            ) = (fi_ema_curr, fi_ema_prev, price_curr, price_ema_curr_val, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Trend context: Price > EMA(Price)
                let is_uptrend = price > price_ema_c;
                let is_downtrend = price < price_ema_c;

                // Signal: Force Index EMA crosses zero
                // Bullish Entry: Uptrend AND Force Index EMA crosses above zero
                if is_uptrend && fi_ema_p <= 0.0 && fi_ema_c > 0.0 {
                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short (Buy to cover)
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("FI Crossover Up: FI EMA {:.2} > 0", fi_ema_c),
                        timestamp_ms: timestamp,
                    });

                    // Enter Long
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
                        reason: format!("FI Crossover Up: FI EMA {:.2} > 0", fi_ema_c),
                        timestamp_ms: timestamp,
                    });
                }
                // Bearish Entry: Downtrend AND Force Index EMA crosses below zero
                else if is_downtrend && fi_ema_p >= 0.0 && fi_ema_c < 0.0 {
                    // Exit any Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("FI Crossover Down: FI EMA {:.2} < 0", fi_ema_c),
                        timestamp_ms: timestamp,
                    });

                    // Enter Short
                    let sl = price_dec + (atr_dec * sl_mult);
                    let risk = sl - price_dec;
                    let tp = price_dec - (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Entry Short
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!("FI Crossover Down: FI EMA {:.2} < 0", fi_ema_c),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: ForceIndexTrendConfig = serde_json::from_value(params)?;
        new_config.validate()?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_force_index_signals() -> Result<()> {
        let config = ForceIndexTrendConfig {
            fi_period: 1,       // raw FI
            fi_ema_period: 2,   // SMA of FI
            price_ema_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = ForceIndexTrend::new(config);

        // We need an uptrend (Price > EMA(2)) and FI EMA crossing above 0
        // Data points needed:
        // i=0: P=10, V=100. FI=None. EMA(Price)=None.
        // i=1: P=11, V=100. FI=(11-10)*100 = 100. EMA(P)=None.
        // i=2: P=12, V=100. FI=(12-11)*100 = 100. EMA(P)=(11+12)/2 = 11.5. FI EMA(2)=(100+100)/2 = 100.

        // This is always positive. Let's make FI go negative then positive.
        // i=0: P=10, V=100.
        // i=1: P=9,  V=100. FI=(9-10)*100 = -100.
        // i=2: P=8,  V=100. FI=(8-9)*100 = -100. FI_EMA=(-100+-100)/2 = -100. EMA(P)=(9+8)/2=8.5
        // i=3: P=10, V=100. FI=(10-8)*100 = 200. FI_EMA=(-100+200)/2 = 50.   EMA(P)=(8+10)/2=9.0.

        // At i=3:
        // Price (10) > EMA(Price) (9.0) -> Uptrend!
        // FI EMA prev (i=2) = -100 (<= 0).
        // FI EMA curr (i=3) = 50 (> 0).
        // Should trigger Entry Long!

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "close" => &[10.0, 9.0, 8.0, 10.0],
            "high"  => &[10.5, 9.5, 8.5, 10.5],
            "low"   => &[9.5, 8.5, 7.5, 7.5],
            "volume"=> &[100.0, 100.0, 100.0, 100.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Should have Entry Long at 4000.
        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.timestamp_ms, 4000);
        assert!(entry.take_profit.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_force_index_signals_short() -> Result<()> {
        let config = ForceIndexTrendConfig {
            fi_period: 1,
            fi_ema_period: 2,
            price_ema_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = ForceIndexTrend::new(config);

        // We need a downtrend (Price < EMA) and FI EMA crossing below 0
        // i=0: P=10, V=100.
        // i=1: P=11, V=100. FI=(11-10)*100 = 100.
        // i=2: P=12, V=100. FI=(12-11)*100 = 100. FI_EMA=100. EMA(P)=11.5.
        // i=3: P=10, V=100. FI=(10-12)*100 = -200. FI_EMA=(100+-200)/2 = -50. EMA(P)=(11+10)/2=10.5.
        // At i=3: Price (10) < EMA(Price) (10.5) -> Downtrend.
        // FI EMA prev (100) >= 0. FI EMA curr (-50) < 0.
        // Should trigger Entry Short!

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "close" => &[10.0, 11.0, 12.0, 10.0],
            "high"  => &[10.5, 11.5, 12.5, 10.5],
            "low"   => &[9.5, 10.5, 11.5, 9.5],
            "volume"=> &[100.0, 100.0, 100.0, 100.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "sell")
            .collect();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.timestamp_ms, 4000);
        assert!(entry.take_profit.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_edge_cases() -> Result<()> {
        let config = ForceIndexTrendConfig {
            fi_period: 1,
            fi_ema_period: 2,
            price_ema_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = ForceIndexTrend::new(config);

        // Test with empty data
        let empty_df = DataFrame::empty();
        let res = strategy.generate_signals(&empty_df).await;
        assert!(res.is_err());

        // Test with data that has no variance (should not panic and should not generate signals)
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "close" => &[10.0, 10.0, 10.0, 10.0],
            "high"  => &[10.0, 10.0, 10.0, 10.0],
            "low"   => &[10.0, 10.0, 10.0, 10.0],
            "volume"=> &[100.0, 100.0, 100.0, 100.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut config = ForceIndexTrendConfig {
            fi_period: 1,
            fi_ema_period: 2,
            price_ema_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };

        assert!(config.validate().is_ok());

        config.fi_period = 0;
        assert!(config.validate().is_err());
        config.fi_period = 1;

        config.price_ema_period = 0;
        assert!(config.validate().is_err());

        Ok(())
    }
}
