//! Williams %R Momentum Strategy.
//!
//! This module implements a mean-reversion strategy based on the Williams %R indicator.
//! Williams %R is a momentum indicator that moves between 0 and -100 and measures overbought
//! and oversold levels. The strategy triggers long positions when the asset is oversold
//! and short positions when the asset is overbought.
//!
//! The strategy utilizes an Average True Range (ATR) based stop loss to manage risk dynamically
//! according to current market volatility.

use crate::indicators::{atr, williams_r};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Configuration parameters for the `WilliamsR` strategy.
///
/// # Examples
///
/// ```rust
/// use strategies::williams_r::WilliamsRConfig;
///
/// let config = WilliamsRConfig {
///     period: 14,
///     oversold_threshold: -80.0,
///     overbought_threshold: -20.0,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     symbol: "BTCUSD".to_string(),
/// };
///
/// assert_eq!(config.period, 14);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WilliamsRConfig {
    /// The lookback period for calculating the Williams %R indicator.
    pub period: usize,
    /// The threshold below which the asset is considered oversold (e.g., -80.0), triggering a buy signal.
    pub oversold_threshold: f64,
    /// The threshold above which the asset is considered overbought (e.g., -20.0), triggering a sell signal.
    pub overbought_threshold: f64,
    /// The multiplier applied to the ATR to calculate the trailing stop loss distance.
    pub stop_loss_atr_mult: f64,
    /// The lookback period for calculating the Average True Range (ATR).
    pub atr_period: usize,
    /// The market symbol this strategy is targeting (e.g., "BTCUSD").
    pub symbol: String,
}

impl StrategyConfig for WilliamsRConfig {}

/// A mean-reversion strategy based on the Williams %R indicator.
///
/// The Williams %R indicator is used to identify overbought and oversold conditions
/// in the market. The strategy generates signals when the indicator crosses specific thresholds.
///
/// # Examples
///
/// ```rust
/// use strategies::williams_r::{WilliamsR, WilliamsRConfig};
///
/// let config = WilliamsRConfig {
///     period: 14,
///     oversold_threshold: -80.0,
///     overbought_threshold: -20.0,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     symbol: "ETHUSD".to_string(),
/// };
///
/// let strategy = WilliamsR::new(config);
/// ```
pub struct WilliamsR {
    config: WilliamsRConfig,
}

impl WilliamsR {
    pub fn new(config: WilliamsRConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for WilliamsR {
    fn name(&self) -> &str {
        "WilliamsR"
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

        // Calculate Williams %R
        let wr_series = williams_r::calculate(data, self.config.period)?;
        let wr_arr = wr_series.f64()?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let wr_curr = wr_arr.get(i);
            let wr_prev = wr_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (Some(wr_c), Some(wr_p), Some(price), Some(atr_val)) =
                (wr_curr, wr_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Long Entry: %R crosses ABOVE oversold threshold (e.g., -80)
                if wr_p <= self.config.oversold_threshold && wr_c > self.config.oversold_threshold {
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
                            "Williams %R Crossover Up: {:.2} > {:.2}",
                            wr_c, self.config.oversold_threshold
                        ),
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
                        reason: format!(
                            "Williams %R Crossover Up: {:.2} > {:.2}",
                            wr_c, self.config.oversold_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Short Entry / Long Exit: %R crosses BELOW overbought threshold (e.g., -20)
                else if wr_p >= self.config.overbought_threshold
                    && wr_c < self.config.overbought_threshold
                {
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
                            "Williams %R Crossover Down: {:.2} < {:.2}",
                            wr_c, self.config.overbought_threshold
                        ),
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
                        reason: format!(
                            "Williams %R Crossover Down: {:.2} < {:.2}",
                            wr_c, self.config.overbought_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: WilliamsRConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_williams_r_signals() -> Result<()> {
        let config = WilliamsRConfig {
            period: 3,
            oversold_threshold: -80.0,
            overbought_threshold: -20.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = WilliamsR::new(config);

        // Build a DataFrame that triggers %R conditions
        // %R = (HH - Close) / (HH - LL) * -100

        // Let's manually engineer the data
        // i=0: H=10, L=8, C=9. HH=10, LL=8.
        // i=1: H=10, L=8, C=9. HH=10, LL=8.
        // i=2: H=10, L=8, C=8.2. HH=10, LL=8. %R = (10 - 8.2)/2 * -100 = 1.8/2 * -100 = -90.0 (Oversold!)
        // i=3: H=10, L=8, C=9. HH=10, LL=8. %R = (10 - 9)/2 * -100 = 1/2 * -100 = -50.0 (Crossed above -80 -> LONG ENTRY)
        // i=4: H=12, L=8, C=11.6. HH=12, LL=8. %R = (12 - 11.6)/4 * -100 = 0.4/4 * -100 = -10.0 (Overbought!)
        // i=5: H=12, L=8, C=10. HH=12, LL=8. %R = (12 - 10)/4 * -100 = 2/4 * -100 = -50.0 (Crossed below -20 -> SHORT ENTRY)

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000],
            "open"  => &[9.0, 9.0, 9.0, 8.2, 9.0, 11.6],
            "high"  => &[10.0, 10.0, 10.0, 10.0, 12.0, 12.0],
            "low"   => &[8.0, 8.0, 8.0, 8.0, 8.0, 8.0],
            "close" => &[9.0, 9.0, 8.2, 9.0, 11.6, 10.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Entry Long at i=3 (4000ms): %R crosses above -80
        let entries_long: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();
        assert_eq!(entries_long.len(), 1);
        assert_eq!(entries_long[0].timestamp_ms, 4000);

        // Entry Short at i=5 (6000ms): %R crosses below -20
        let entries_short: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "sell")
            .collect();
        assert_eq!(entries_short.len(), 1);
        assert_eq!(entries_short[0].timestamp_ms, 6000);

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_update() -> Result<()> {
        let mut strategy = WilliamsR::new(WilliamsRConfig {
            period: 14,
            oversold_threshold: -80.0,
            overbought_threshold: -20.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "period": 20,
            "oversold_threshold": -70.0,
            "overbought_threshold": -30.0,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 20,
            "symbol": "NEW_TEST"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.period, 20);
        assert_eq!(strategy.config.oversold_threshold, -70.0);
        assert_eq!(strategy.config.symbol, "NEW_TEST");

        Ok(())
    }
}
