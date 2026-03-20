//! On-Balance Volume (OBV) Trend Following Strategy.
//!
//! This module implements a volume-based strategy designed to confirm price trends
//! and identify potential breakouts before they happen.
//!
//! # The Story
//! Price is what you pay, but volume is the effort behind the move.
//! On-Balance Volume adds volume on up days and subtracts it on down days.
//! By comparing the OBV line against its own Simple Moving Average (SMA), we can see
//! if the "smart money" is accumulating or distributing the asset.
//!
//! - **Bullish Signal:** When OBV crosses *above* its SMA, buying pressure is increasing.
//! - **Bearish Signal:** When OBV crosses *below* its SMA, selling pressure is taking over.
//!
//! This strategy uses these volume momentum shifts to generate entries and exits.

use crate::indicators::{atr, obv, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Configuration for the [`ObvTrendFollowing`] strategy.
///
/// Defines the period for the OBV's signal line (SMA), as well as risk management settings.
///
/// # Examples
///
/// ```rust
/// use strategies::obv_trend::ObvTrendFollowingConfig;
///
/// let json = r#"{
///     "obv_sma_period": 20,
///     "stop_loss_atr_mult": 2.0,
///     "atr_period": 14,
///     "symbol": "ETHUSD"
/// }"#;
///
/// let config: ObvTrendFollowingConfig = serde_json::from_str(json).unwrap();
/// assert_eq!(config.obv_sma_period, 20);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObvTrendFollowingConfig {
    pub obv_sma_period: usize, // Signal line period
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for ObvTrendFollowingConfig {}

/// The OBV Trend Following strategy implementation.
///
/// Generates signals when the On-Balance Volume indicator crosses its own Simple Moving Average.
///
/// # Examples
///
/// ```rust
/// use strategies::obv_trend::{ObvTrendFollowing, ObvTrendFollowingConfig};
/// use strategies::strategy::Strategy;
///
/// let config = ObvTrendFollowingConfig {
///     obv_sma_period: 20,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     symbol: "ETHUSD".to_string(),
/// };
///
/// let strategy = ObvTrendFollowing::new(config);
/// assert_eq!(strategy.name(), "ObvTrendFollowing");
/// ```
///
/// # Errors
///
/// This strategy will return an error (but not panic) if the input `DataFrame`
/// lacks the required columns (`close`, `volume`, `high`, `low`, `timestamp_unix_ms`).
pub struct ObvTrendFollowing {
    config: ObvTrendFollowingConfig,
}

impl ObvTrendFollowing {
    pub fn new(config: ObvTrendFollowingConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for ObvTrendFollowing {
    fn name(&self) -> &str {
        "ObvTrendFollowing"
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

        // Calculate OBV
        let obv_series = obv::calculate(data)?;
        let obv_arr = obv_series.f64()?;

        // Calculate OBV SMA (Signal Line)
        // We create a temporary DataFrame where "close" is the OBV series
        // because sma::calculate expects a DataFrame with a "close" column.
        let obv_df = DataFrame::new(vec![Series::new("close", obv_arr.clone())])?;

        let obv_sma_series = sma::calculate(&obv_df, self.config.obv_sma_period)?;
        let obv_sma_arr = obv_sma_series.f64()?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let obv_curr = obv_arr.get(i);
            let obv_prev = obv_arr.get(i - 1);
            let sma_curr = obv_sma_arr.get(i);
            let sma_prev = obv_sma_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(obv_c),
                Some(obv_p),
                Some(sma_c),
                Some(sma_p),
                Some(price),
                Some(atr_val),
            ) = (obv_curr, obv_prev, sma_curr, sma_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Bullish Crossover (OBV crosses ABOVE SMA)
                if obv_p <= sma_p && obv_c > sma_c {
                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short (Buy to cover)
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("OBV Crossover Up: OBV {:.2} > SMA {:.2}", obv_c, sma_c),
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
                        reason: format!("OBV Crossover Up: OBV {:.2} > SMA {:.2}", obv_c, sma_c),
                        timestamp_ms: timestamp,
                    });
                }
                // Bearish Crossover (OBV crosses BELOW SMA)
                else if obv_p >= sma_p && obv_c < sma_c {
                    // Exit any Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("OBV Crossover Down: OBV {:.2} < SMA {:.2}", obv_c, sma_c),
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
                        reason: format!("OBV Crossover Down: OBV {:.2} < SMA {:.2}", obv_c, sma_c),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: ObvTrendFollowingConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_obv_signals() -> Result<()> {
        let config = ObvTrendFollowingConfig {
            obv_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = ObvTrendFollowing::new(config);

        // We need data that creates an OBV line that crosses its SMA.
        // OBV starts at 0.
        // SMA(2) lags OBV.

        // Data:
        // 0: Price 10, Vol 100. OBV=0. SMA(2)=Null.
        // 1: Price 11, Vol 100. OBV=100. SMA(2)=Null.
        // 2: Price 12, Vol 100. OBV=200. SMA(2)=(100+200)/2=150. (OBV 200 > 150) -> Maybe crossover?
        // Wait, at i=2, prev OBV=100, prev SMA=Null?
        // SMA logic: if period=2.
        // i=0: win=[0], sum=0.
        // i=1: win=[0, 100], sum=100. SMA=50.
        // So at i=1: OBV=100, SMA=50. Prev OBV=0. Prev SMA=Null?

        // Let's use simpler logic.
        // If we want a crossover up: OBV should go from below SMA to above.

        // Let's create a scenario:
        // OBV: 10, 10, 20 (Jump up)
        // SMA(2): -, 10, 15
        // i=1: OBV=10, SMA=10.
        // i=2: OBV=20, SMA=15.
        // Cross Up? 10 <= 10 && 20 > 15. YES.

        // How to achieve OBV 10, 10, 20?
        // i=0: OBV=0 (init).
        // Need OBV at i=0 to be meaningful? No, standard is 0.
        // Let's just create raw data that results in OBV values we want.

        // i=0: P=10, V=10. OBV=0.
        // i=1: P=11, V=10. OBV=10. (Change +10).
        // i=2: P=11, V=0.  OBV=10. (Change 0).
        // i=3: P=12, V=20. OBV=30. (Change +20).

        // SMA(2):
        // i=0: -
        // i=1: (0+10)/2 = 5.
        // i=2: (10+10)/2 = 10.
        // i=3: (10+30)/2 = 20.

        // Check i=3:
        // Prev (i=2): OBV=10, SMA=10. (10 <= 10)
        // Curr (i=3): OBV=30, SMA=20. (30 > 20)
        // Condition: Prev <= SMA_Prev && Curr > SMA_Curr.
        // 10 <= 10 && 30 > 20. TRUE.

        // Should trigger Entry Long.

        // Need ATR too.
        // High/Low needed for ATR.
        // i=0: H10.5 L9.5 C10. TR=1.
        // i=1: H11.5 L10.5 C11. TR=1.
        // i=2: H11.5 L10.5 C11. TR=1.
        // i=3: H12.5 L11.5 C12. TR=1.5. (H=12.5, PC=11).
        // ATR(2) at i=3 is calculated to be 1.3125 (per test failure).

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "close" => &[10.0, 11.0, 11.0, 12.0],
            "high"  => &[10.5, 11.5, 11.5, 12.5],
            "low"   => &[9.5, 10.5, 10.5, 11.5],
            "volume"=> &[10.0, 10.0, 0.0, 20.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Should have Entry Long at 4000.
        // i=3 is 4000.

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.timestamp_ms, 4000);
        assert!(entry.take_profit.is_some());

        // Check TP calculation
        // Entry Price = 12.0.
        // ATR = 1.3125 (Approx). Mult = 2.0. SL Dist = 2.625. SL = 9.375.
        // Risk = 2.625.
        // TP = 12.0 + (2.625 * 2) = 17.25.

        let tp = entry.take_profit.unwrap();
        assert!((tp - 17.25).abs() < 0.001, "TP was {}, expected 17.25", tp);

        Ok(())
    }
}
