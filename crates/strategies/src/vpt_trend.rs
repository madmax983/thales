//! The Volume Price Trend (VPT) Strategy
//!
//! The Volume Price Trend (VPT) combines price and volume to determine the balance between supply and demand.
//! It is similar to On-Balance Volume (OBV) but adjusts the volume added or subtracted by the percentage change in the price trend.
//!
//! - **Entry Signal:** A buy signal is generated when the VPT crosses above its Simple Moving Average (SMA).
//! - **Exit Signal:** A sell signal is generated when the VPT crosses below its SMA.
//!
use crate::indicators::{atr, sma, vpt};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

/// Configuration for the Volume Price Trend (VPT) trend-following strategy.
/// Configuration parameters for the `VptTrend` strategy.
///
/// # Examples
///
/// ```
/// use strategies::vpt_trend::VptTrendConfig;
///
/// let config = VptTrendConfig {
///     vpt_sma_period: 21,
///     price_sma_period: 21,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     symbol: "BTCUSD".to_string(),
/// };
/// ```
#[derive(Debug, Clone, serde::Deserialize)]
pub struct VptTrendConfig {
    /// Period for the Simple Moving Average (SMA) of the VPT indicator.
    pub vpt_sma_period: usize,
    /// Period for the Simple Moving Average (SMA) of the closing price.
    pub price_sma_period: usize,
    /// ATR multiplier for stop-loss calculation.
    pub stop_loss_atr_mult: f64,
    /// Period for the Average True Range (ATR) indicator.
    pub atr_period: usize,
    /// The trading symbol.
    pub symbol: String,
}

impl StrategyConfig for VptTrendConfig {}

impl VptTrendConfig {
    pub fn validate(&self) -> Result<()> {
        if self.vpt_sma_period == 0 || self.price_sma_period == 0 || self.atr_period == 0 {
            anyhow::bail!("Periods must be > 0");
        }
        Ok(())
    }
}

/// A trend-following strategy based on the Volume Price Trend (VPT) indicator.
/// The VPT Trend strategy implementation.
///
/// # Examples
///
/// ```
/// use strategies::vpt_trend::{VptTrend, VptTrendConfig};
/// use strategies::strategy::Strategy;
///
/// let config = VptTrendConfig {
///     vpt_sma_period: 21,
///     price_sma_period: 21,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     symbol: "BTCUSD".to_string(),
/// };
///
/// let strategy = VptTrend::new(config);
/// assert_eq!(strategy.name(), "VptTrend");
/// ```
pub struct VptTrend {
    config: VptTrendConfig,
}

impl VptTrend {
    pub fn new(config: VptTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for VptTrend {
    fn name(&self) -> &str {
        "VptTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.i64()?;

        let close_series = data.column("close")?;
        let close_arr = close_series.f64()?;

        // Calculate VPT
        let vpt_series = vpt::calculate(data)?;

        // Create DataFrame with VPT for SMA calculation
        let mut vpt_df_for_sma = DataFrame::new(vec![vpt_series.clone()])?;

        // Calculate SMA of VPT
        // Rename column to "close" as SMA indicator expects "close" column
        vpt_df_for_sma.rename("vpt", "close")?;

        let vpt_sma_series = sma::calculate(&vpt_df_for_sma, self.config.vpt_sma_period)?;
        let vpt_sma_arr = vpt_sma_series.f64()?;
        let vpt_arr = vpt_series.f64()?;

        // Calculate SMA of Price
        let price_sma_series = sma::calculate(data, self.config.price_sma_period)?;
        let price_sma_arr = price_sma_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();

        for i in 1..close_arr.len() {
            let prev_vpt = vpt_arr.get(i - 1);
            let prev_vpt_sma = vpt_sma_arr.get(i - 1);
            let curr_vpt = vpt_arr.get(i);
            let curr_vpt_sma = vpt_sma_arr.get(i);
            let curr_price = close_arr.get(i);
            let curr_price_sma = price_sma_arr.get(i);
            let curr_atr = atr_arr.get(i);
            let time_ms = time_arr.get(i);

            if let (
                Some(p_vpt),
                Some(p_vpt_sma),
                Some(c_vpt),
                Some(c_vpt_sma),
                Some(price),
                Some(p_sma),
                Some(atr_val),
                Some(ts),
            ) = (
                prev_vpt,
                prev_vpt_sma,
                curr_vpt,
                curr_vpt_sma,
                curr_price,
                curr_price_sma,
                curr_atr,
                time_ms,
            ) {
                // Buy: VPT crosses above its SMA and Price > Price SMA
                if p_vpt <= p_vpt_sma && c_vpt > c_vpt_sma && price > p_sma {
                    let stop_loss = price - (atr_val * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: "VPT crossed above SMA and price > SMA".to_string(),
                        timestamp_ms: ts,
                    });
                }
                // Sell Short: VPT crosses below its SMA and Price < Price SMA
                else if p_vpt >= p_vpt_sma && c_vpt < c_vpt_sma && price < p_sma {
                    let stop_loss = price + (atr_val * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: "VPT crossed below SMA and price < SMA".to_string(),
                        timestamp_ms: ts,
                    });
                }

                // Exit Long: VPT crosses below its SMA
                if p_vpt >= p_vpt_sma && c_vpt < c_vpt_sma {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "VPT crossed below SMA (Exit Long)".to_string(),
                        timestamp_ms: ts,
                    });
                }

                // Exit Short: VPT crosses above its SMA
                if p_vpt <= p_vpt_sma && c_vpt > c_vpt_sma {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "VPT crossed above SMA (Exit Short)".to_string(),
                        timestamp_ms: ts,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: VptTrendConfig = serde_json::from_value(params)?;
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
    async fn test_vpt_trend_buy_signal() -> Result<()> {
        // We need enough data to prime the SMA(2) and ATR(2), and to get a cross.
        // vpt starts at null.
        // i=0: c=10, v=100 -> vpt=null
        // i=1: c=11, v=50 -> vpt=5
        // i=2: c=10.5, v=20 -> vpt=4.09
        // vpt_sma(2) at i=2: (5 + 4.09)/2 = 4.545 -> p_vpt (4.09) <= p_vpt_sma (4.545)
        // i=3: c=12, v=100 -> vpt=4.09 + 100*(1.5/10.5) = 18.37 -> c_vpt (18.37)
        // vpt_sma(2) at i=3: (4.09 + 18.37)/2 = 11.23 -> c_vpt (18.37) > c_vpt_sma (11.23)
        // price_sma(2) at i=2: (11 + 10.5)/2 = 10.75
        // c_price = 12, c_price > p_sma(10.75) => Buy!

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "high" => &[10.0, 11.0, 11.0, 13.0],
            "low" => &[9.0, 10.0, 10.0, 10.5],
            "close" => &[10.0, 11.0, 10.5, 12.0],
            "volume" => &[100.0, 50.0, 20.0, 100.0]
        )?;

        let config = VptTrendConfig {
            vpt_sma_period: 2,
            price_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };

        let strategy = VptTrend::new(config);
        let signals = strategy.generate_signals(&df).await?;

        let entry = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "buy");
        assert!(entry.is_some(), "Expected a buy entry signal");

        Ok(())
    }

    #[tokio::test]
    async fn test_vpt_trend_sell_signal() -> Result<()> {
        // i=0: c=12, v=100 -> vpt=null
        // i=1: c=11, v=50 -> vpt = 50 * (-1/12) = -4.16
        // i=2: c=11.5, v=20 -> vpt = -4.16 + 20*(0.5/11) = -3.25
        // vpt_sma(2) at i=2: (-4.16 + -3.25)/2 = -3.705 -> p_vpt(-3.25) >= p_vpt_sma(-3.705)
        // i=3: c=10, v=100 -> vpt = -3.25 + 100*(-1.5/11.5) = -16.29
        // vpt_sma(2) at i=3: (-3.25 + -16.29)/2 = -9.77 -> c_vpt(-16.29) < c_vpt_sma(-9.77)
        // price_sma(2) at i=2: (11 + 11.5)/2 = 11.25
        // c_price = 10 < p_sma(11.25) => Sell Short!

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "high" => &[13.0, 12.0, 12.0, 10.5],
            "low" => &[11.0, 10.0, 10.0, 9.0],
            "close" => &[12.0, 11.0, 11.5, 10.0],
            "volume" => &[100.0, 50.0, 20.0, 100.0]
        )?;

        let config = VptTrendConfig {
            vpt_sma_period: 2,
            price_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };

        let strategy = VptTrend::new(config);
        let signals = strategy.generate_signals(&df).await?;

        let entry = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "sell");
        assert!(entry.is_some(), "Expected a sell entry signal");

        Ok(())
    }

    #[tokio::test]
    async fn test_vpt_trend_empty_data() -> Result<()> {
        let df = DataFrame::default();
        let config = VptTrendConfig {
            vpt_sma_period: 2,
            price_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = VptTrend::new(config);
        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_vpt_trend_parameter_validation() -> Result<()> {
        let mut strategy = VptTrend::new(VptTrendConfig {
            vpt_sma_period: 2,
            price_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        });

        let valid_params = serde_json::json!({
            "vpt_sma_period": 3,
            "price_sma_period": 10,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 5,
            "symbol": "BTCUSD"
        });

        assert!(strategy.update_params(valid_params).await.is_ok());

        let invalid_params = serde_json::json!({
            "vpt_sma_period": 0,
            "price_sma_period": 10,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 5,
            "symbol": "BTCUSD"
        });

        assert!(strategy.update_params(invalid_params).await.is_err());

        Ok(())
    }
}
