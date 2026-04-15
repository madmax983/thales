//! The VWAP + CCI Trend Strategy
//!
//! This strategy uses the Volume Weighted Average Price (VWAP) as a primary trend filter and the Commodity Channel Index (CCI) for timing entries and exits.
//!
//! - **Entry Signal:** A buy signal is generated when the price is above the VWAP (indicating an uptrend) and the CCI crosses above a specified buy threshold (e.g., 100), indicating strong momentum.
//! - **Exit Signal:** A sell signal is generated when the price falls below the VWAP or the CCI drops below a sell threshold (e.g., -100).
//!
use crate::indicators::{atr, cci, vwap};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration parameters for the `VwapCciTrend` strategy.
///
/// # Examples
///
/// ```
/// use strategies::vwap_cci_trend::VwapCciTrendConfig;
///
/// let config = VwapCciTrendConfig {
///     cci_period: 20,
///     cci_buy_threshold: 100.0,
///     cci_sell_threshold: -100.0,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     symbol: "BTCUSD".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VwapCciTrendConfig {
    pub cci_period: usize,
    pub cci_buy_threshold: f64,
    pub cci_sell_threshold: f64,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}

impl Default for VwapCciTrendConfig {
    fn default() -> Self {
        Self {
            cci_period: 20,
            cci_buy_threshold: 100.0,
            cci_sell_threshold: -100.0,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for VwapCciTrendConfig {}

/// The VWAP + CCI Trend strategy implementation.
///
/// # Examples
///
/// ```
/// use strategies::vwap_cci_trend::{VwapCciTrend, VwapCciTrendConfig};
/// use strategies::strategy::Strategy;
///
/// let config = VwapCciTrendConfig {
///     cci_period: 20,
///     cci_buy_threshold: 100.0,
///     cci_sell_threshold: -100.0,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     symbol: "BTCUSD".to_string(),
/// };
///
/// let strategy = VwapCciTrend::new(config);
/// assert_eq!(strategy.name(), "VwapCciTrend");
/// ```
pub struct VwapCciTrend {
    config: VwapCciTrendConfig,
}

impl VwapCciTrend {
    pub fn new(config: VwapCciTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for VwapCciTrend {
    fn name(&self) -> &str {
        "VwapCciTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.cci_period + 1 {
            return Ok(vec![]);
        }

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let close_series = data.column("close")?.f64()?;

        let vwap_series = vwap::calculate(data)?;
        let vwap_arr = vwap_series.f64()?;

        let cci_series = cci::calculate(data, self.config.cci_period)?;
        let cci_arr = cci_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let vwap_vec: Vec<Option<f64>> = vwap_arr.into_iter().collect();
        let cci_vec: Vec<Option<f64>> = cci_arr.into_iter().collect();
        let close_vec: Vec<Option<f64>> = close_series.into_iter().collect();
        let atr_vec: Vec<Option<f64>> = atr_arr.into_iter().collect();

        let mut signals = Vec::new();
        let len = data.height();

        for i in self.config.cci_period..len {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price = close_vec.get(i).copied().flatten().unwrap_or(0.0);
            let prev_price = close_vec.get(i - 1).copied().flatten().unwrap_or(0.0);

            let curr_vwap = vwap_vec.get(i).copied().flatten();
            let prev_vwap = vwap_vec.get(i - 1).copied().flatten();
            let curr_cci = cci_vec.get(i).copied().flatten();
            let prev_cci = cci_vec.get(i - 1).copied().flatten();
            let curr_atr = atr_vec.get(i).copied().flatten();

            if let (Some(vwap), Some(p_vwap), Some(cci_val), Some(p_cci), Some(atr_val)) =
                (curr_vwap, prev_vwap, curr_cci, prev_cci, curr_atr)
            {
                // Entry Long: Price > VWAP and CCI crosses above buy_threshold
                if price > vwap
                    && p_cci <= self.config.cci_buy_threshold
                    && cci_val > self.config.cci_buy_threshold
                {
                    let sl = price - (self.config.stop_loss_atr_mult * atr_val);
                    let tp = price + (2.0 * self.config.stop_loss_atr_mult * atr_val);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: Some(tp),
                        reason: "VWAP CCI Bullish Confirmation".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Entry Short: Price < VWAP and CCI crosses below sell_threshold
                if price < vwap
                    && p_cci >= self.config.cci_sell_threshold
                    && cci_val < self.config.cci_sell_threshold
                {
                    let sl = price + (self.config.stop_loss_atr_mult * atr_val);
                    let tp = price - (2.0 * self.config.stop_loss_atr_mult * atr_val);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: Some(tp),
                        reason: "VWAP CCI Bearish Confirmation".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Exit Long: Price crosses below VWAP or CCI crosses below 0
                if (prev_price >= p_vwap && price < vwap) || (p_cci >= 0.0 && cci_val < 0.0) {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "VWAP CCI Long Exit".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Exit Short: Price crosses above VWAP or CCI crosses above 0
                if (prev_price <= p_vwap && price > vwap) || (p_cci <= 0.0 && cci_val > 0.0) {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "VWAP CCI Short Exit".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: VwapCciTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_vwap_cci_trend_empty_data() -> Result<()> {
        let config = VwapCciTrendConfig::default();
        let strategy = VwapCciTrend::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[] as &[i64],
            "open" => &[] as &[f64],
            "high" => &[] as &[f64],
            "low" => &[] as &[f64],
            "close" => &[] as &[f64],
            "volume" => &[] as &[f64]
        )?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_vwap_cci_trend_parameter_validation() -> Result<()> {
        let mut strategy = VwapCciTrend::new(VwapCciTrendConfig::default());

        assert_eq!(strategy.name(), "VwapCciTrend");
        assert_eq!(strategy.strategy_type(), StrategyType::TrendFollowing);

        let new_params = serde_json::json!({
            "cci_period": 21,
            "cci_buy_threshold": 110.0,
            "cci_sell_threshold": -110.0,
            "atr_period": 10,
            "stop_loss_atr_mult": 1.5,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.cci_period, 21);
        assert_eq!(strategy.config.cci_buy_threshold, 110.0);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }

    #[tokio::test]
    async fn test_vwap_cci_trend_signals() -> Result<()> {
        let config = VwapCciTrendConfig {
            cci_period: 2,
            cci_buy_threshold: 50.0,   // lower threshold for mock tests
            cci_sell_threshold: -50.0, // lower threshold for mock tests
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = VwapCciTrend::new(config);

        let timestamps: Vec<i64> = (0..20).map(|i| 1000 + i as i64 * 1000).collect();
        let mut closes = vec![100.0; 20];

        // We must ensure CCI triggers > 100. CCI is (Typical Price - SMA(TP)) / (0.015 * Mean Deviation)
        // With period=2, we need a sharp movement.
        closes[0] = 100.0;
        closes[1] = 100.0;
        closes[2] = 150.0; // Huge jump to ensure CCI crosses above 100
        closes[3] = 160.0;
        closes[4] = 160.0;

        closes[5] = 50.0; // Huge drop to cross VWAP and CCI < -100
        closes[6] = 40.0;

        let highs = closes.iter().map(|c| c + 2.0).collect::<Vec<_>>();
        let lows = closes.iter().map(|c| c - 2.0).collect::<Vec<_>>();
        let volumes = vec![1000.0; 20];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "open" => &closes,
            "high" => &highs,
            "low" => &lows,
            "close" => &closes,
            "volume" => volumes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();

        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        assert!(!entries.is_empty(), "Should generate entry signals");
        assert!(!exits.is_empty(), "Should generate exit signals");

        // Verify Stop Loss and Take Profit
        if let Some(entry) = entries.first() {
            assert!(entry.stop_loss.is_some());
            assert!(entry.take_profit.is_some());
        }

        Ok(())
    }
}
