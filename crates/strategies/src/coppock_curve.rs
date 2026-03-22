use crate::indicators::{atr, roc, wma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Configuration parameters for the `CoppockCurve` strategy.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CoppockCurveConfig {
    /// The period for the longer Rate of Change (ROC).
    pub roc_long_period: usize,
    /// The period for the shorter Rate of Change (ROC).
    pub roc_short_period: usize,
    /// The period for the Weighted Moving Average (WMA).
    pub wma_period: usize,
    /// The multiplier applied to the ATR to calculate the trailing stop loss distance.
    pub stop_loss_atr_mult: f64,
    /// The lookback period for calculating the Average True Range (ATR).
    pub atr_period: usize,
    /// The maximum position size to allocate for a trade.
    pub max_position_size: f64,
    /// The market symbol this strategy is targeting (e.g., "BTCUSD").
    pub symbol: String,
}

impl Default for CoppockCurveConfig {
    fn default() -> Self {
        Self {
            roc_long_period: 14,
            roc_short_period: 11,
            wma_period: 10,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        }
    }
}

impl StrategyConfig for CoppockCurveConfig {}

/// A trend-following strategy based on the Coppock Curve.
///
/// The Coppock Curve is calculated as a 10-period WMA of the sum of a 14-period ROC and an 11-period ROC.
/// It is primarily used to identify major market bottoms and tops. The strategy generates a buy signal when
/// the curve crosses above zero, and a sell signal when it crosses below zero.
///
/// # Examples
///
/// ```rust
/// use strategies::coppock_curve::{CoppockCurve, CoppockCurveConfig};
///
/// let config = CoppockCurveConfig {
///     roc_long_period: 14,
///     roc_short_period: 11,
///     wma_period: 10,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     max_position_size: 100.0,
///     symbol: "BTCUSD".to_string(),
/// };
///
/// let strategy = CoppockCurve::new(config).unwrap();
/// ```
pub struct CoppockCurve {
    config: CoppockCurveConfig,
}

impl CoppockCurve {
    pub fn new(config: CoppockCurveConfig) -> Result<Self> {
        if config.roc_long_period == 0 || config.roc_short_period == 0 || config.wma_period == 0 {
            anyhow::bail!("Periods must be greater than 0");
        }
        Ok(Self { config })
    }
}

#[async_trait]
impl Strategy for CoppockCurve {
    fn name(&self) -> &str {
        "CoppockCurve"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.roc_long_period + self.config.wma_period {
            return Ok(vec![]);
        }

        // Calculate ROCs
        let roc_long = roc::calculate(data, self.config.roc_long_period)?;
        let roc_short = roc::calculate(data, self.config.roc_short_period)?;

        let roc_long_arr = roc_long.f64()?;
        let roc_short_arr = roc_short.f64()?;

        // Sum ROCs
        let mut roc_sum_vals = Vec::with_capacity(data.height());
        for i in 0..data.height() {
            let rl = roc_long_arr.get(i);
            let rs = roc_short_arr.get(i);
            if let (Some(l), Some(s)) = (rl, rs) {
                roc_sum_vals.push(Some(l + s));
            } else {
                roc_sum_vals.push(None);
            }
        }
        let roc_sum_series = Series::new("close", roc_sum_vals);
        let roc_sum_df = DataFrame::new(vec![roc_sum_series])?;

        // Calculate WMA of the sum (this is the Coppock Curve)
        let coppock = wma::calculate(&roc_sum_df, self.config.wma_period)?;
        let coppock_arr = coppock.f64()?;

        // Calculate ATR for risk management
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let close_series = data.column("close")?.f64()?;
        let time_series = data.column("timestamp_unix_ms")?.cast(&DataType::Int64)?;
        let time_arr = time_series.i64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        // Let size hint be bounded by max_position_size
        let size_hint_str = format!("{:.2}", self.config.max_position_size);

        for i in 1..data.height() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let cc_curr = coppock_arr.get(i);
            let cc_prev = coppock_arr.get(i - 1);
            let price_opt = close_series.get(i);
            let atr_opt = atr_arr.get(i);

            if let (Some(c_curr), Some(c_prev), Some(price), Some(atr_val)) =
                (cc_curr, cc_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Entry Long: Coppock Curve crosses above zero
                if c_prev <= 0.0 && c_curr > 0.0 {
                    // Exit any existing Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Buy to cover short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Coppock Curve crossed above zero (Trend Reversal Up)".to_string(),
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
                        size_hint: size_hint_str.clone(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: "Coppock Curve crossed above zero (Trend Reversal Up)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
                // Entry Short: Coppock Curve crosses below zero
                else if c_prev >= 0.0 && c_curr < 0.0 {
                    // Exit any existing Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Sell to close long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "Coppock Curve crossed below zero (Trend Reversal Down)"
                            .to_string(),
                        timestamp_ms: timestamp,
                    });

                    // Enter Short
                    let sl = price_dec + (atr_dec * sl_mult);
                    let risk = sl - price_dec;
                    let tp = price_dec - (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: size_hint_str.clone(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: "Coppock Curve crossed below zero (Trend Reversal Down)"
                            .to_string(),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: CoppockCurveConfig = serde_json::from_value(params)?;
        if new_config.roc_long_period == 0
            || new_config.roc_short_period == 0
            || new_config.wma_period == 0
        {
            anyhow::bail!("Periods must be greater than 0");
        }
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_coppock_curve_signal_generation() -> Result<()> {
        let config = CoppockCurveConfig {
            roc_long_period: 3,
            roc_short_period: 2,
            wma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        };

        let strategy = CoppockCurve::new(config)?;

        // Engineer prices to cross below then above zero
        let df = df!(
            "timestamp_unix_ms" => &[
                1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000, 11000, 12000
            ],
            "open"  => &[100.0, 110.0, 120.0, 130.0, 120.0, 110.0, 100.0, 90.0, 80.0, 90.0, 110.0, 130.0],
            "high"  => &[101.0, 111.0, 121.0, 131.0, 121.0, 111.0, 101.0, 91.0, 81.0, 91.0, 111.0, 131.0],
            "low"   => &[99.0, 109.0, 119.0, 129.0, 119.0, 109.0, 99.0, 89.0, 79.0, 89.0, 109.0, 129.0],
            "close" => &[100.0, 110.0, 120.0, 130.0, 120.0, 110.0, 100.0, 90.0, 80.0, 90.0, 110.0, 130.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let short_entry = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "sell");
        assert!(short_entry.is_some(), "Should generate short entry signal");

        let long_entry = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "buy");
        assert!(long_entry.is_some(), "Should generate long entry signal");

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = CoppockCurve::new(CoppockCurveConfig {
            roc_long_period: 14,
            roc_short_period: 11,
            wma_period: 10,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            max_position_size: 100.0,
            symbol: "TEST".to_string(),
        })?;

        // Valid update
        let new_params = serde_json::json!({
            "roc_long_period": 20,
            "roc_short_period": 15,
            "wma_period": 10,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 14,
            "max_position_size": 200.0,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.roc_long_period, 20);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        // Invalid update (0 period)
        let invalid_params = serde_json::json!({
            "roc_long_period": 0,
            "roc_short_period": 11,
            "wma_period": 10,
            "stop_loss_atr_mult": 2.0,
            "atr_period": 14,
            "max_position_size": 100.0,
            "symbol": "TEST"
        });

        let res = strategy.update_params(invalid_params).await;
        assert!(res.is_err());

        Ok(())
    }
}
