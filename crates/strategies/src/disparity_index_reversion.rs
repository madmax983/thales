use polars::prelude::*;
use crate::indicators::{atr, disparity_index};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;

use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Configuration parameters for the `DisparityIndexMeanReversion` strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisparityIndexMeanReversionConfig {
    /// The lookback period for calculating the Disparity Index.
    pub period: usize,
    /// The negative threshold below which the asset is considered oversold.
    pub oversold_threshold: f64,
    /// The positive threshold above which the asset is considered overbought.
    pub overbought_threshold: f64,
    /// The multiplier applied to the ATR to calculate the trailing stop loss distance.
    pub stop_loss_atr_mult: f64,
    /// The lookback period for calculating the Average True Range (ATR).
    pub atr_period: usize,
    /// The market symbol this strategy is targeting.
    pub symbol: String,
    /// The maximum allowed position size constraint.
    pub max_position_size: f64,
}

impl Default for DisparityIndexMeanReversionConfig {
    fn default() -> Self {
        Self {
            period: 14,
            oversold_threshold: -5.0,
            overbought_threshold: 5.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
            max_position_size: 100.0,
        }
    }
}

impl StrategyConfig for DisparityIndexMeanReversionConfig {}

/// A mean reversion strategy based on the Disparity Index.
pub struct DisparityIndexMeanReversion {
    config: DisparityIndexMeanReversionConfig,
}

impl DisparityIndexMeanReversion {
    /// Creates a new instance of the strategy with the provided configuration.
    pub fn new(config: DisparityIndexMeanReversionConfig) -> Result<Self> {
        if config.period == 0 {
            anyhow::bail!("Disparity Index period must be > 0");
        }
        if config.atr_period == 0 {
            anyhow::bail!("ATR period must be > 0");
        }
        if config.oversold_threshold >= config.overbought_threshold {
            anyhow::bail!("Oversold threshold must be strictly less than overbought threshold");
        }
        Ok(Self { config })
    }
}

#[async_trait]
impl Strategy for DisparityIndexMeanReversion {
    fn name(&self) -> &str {
        "DisparityIndexMeanReversion"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.period.max(self.config.atr_period) {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let di_series = disparity_index::calculate(data, self.config.period)?;
        let di_arr = di_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let di_curr = di_arr.get(i);
            let di_prev = di_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (Some(d_c), Some(d_p), Some(price), Some(atr_val)) =
                (di_curr, di_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Long Entry: Disparity Index crosses above oversold_threshold
                if d_p <= self.config.oversold_threshold && d_c > self.config.oversold_threshold {
                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("DI crossed above oversold threshold ({:.2})", d_c),
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
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!("DI oversold reversion: {:.2}", d_c),
                        timestamp_ms: timestamp,
                    });
                }
                // Short Entry: Disparity Index crosses below overbought_threshold
                else if d_p >= self.config.overbought_threshold && d_c < self.config.overbought_threshold {
                    // Exit any Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("DI crossed below overbought threshold ({:.2})", d_c),
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
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!("DI overbought reversion: {:.2}", d_c),
                        timestamp_ms: timestamp,
                    });
                }
                // Long Exit: Disparity Index crosses above 0
                else if d_p <= 0.0 && d_c > 0.0 {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.9,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("DI reverted to mean (crossed above 0): {:.2}", d_c),
                        timestamp_ms: timestamp,
                    });
                }
                // Short Exit: Disparity Index crosses below 0
                else if d_p >= 0.0 && d_c < 0.0 {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.9,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("DI reverted to mean (crossed below 0): {:.2}", d_c),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: DisparityIndexMeanReversionConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    fn create_test_data() -> DataFrame {
        // Disparity Index formula: (Close - SMA) / SMA * 100
        // SMA period = 2.
        // Bar 0: C=100. SMA=null. DI=null.
        // Bar 1: C=100. SMA(100,100)=100. DI=0.
        // Bar 2: C=80.  SMA(100,80)=90. DI=(80-90)/90*100 = -11.11  (Oversold!)
        // Bar 3: C=90.  SMA(80,90)=85.  DI=(90-85)/85*100 = +5.88   (Crosses back above -5.0 -> Long Entry!)
        // Bar 4: C=120. SMA(90,120)=105. DI=(120-105)/105*100 = +14.28 (Overbought!)
        // Bar 5: C=100. SMA(120,100)=110. DI=(100-110)/110*100 = -9.09 (Crosses back below +5.0 -> Short Entry!)

        // Let's add High, Low for ATR.
        let timestamp_unix_ms = Int64Chunked::from_slice("timestamp_unix_ms", &[1000, 2000, 3000, 4000, 5000, 6000]);
        let open = Float64Chunked::from_slice("open", &[100.0, 100.0, 100.0, 80.0, 90.0, 120.0]);
        let high = Float64Chunked::from_slice("high", &[105.0, 105.0, 105.0, 95.0, 125.0, 125.0]);
        let low = Float64Chunked::from_slice("low", &[95.0, 95.0, 75.0, 75.0, 85.0, 95.0]);
        let close = Float64Chunked::from_slice("close", &[100.0, 100.0, 80.0, 90.0, 120.0, 100.0]);
        let volume = Float64Chunked::from_slice("volume", &[100.0, 100.0, 100.0, 100.0, 100.0, 100.0]);

        DataFrame::new(vec![
            timestamp_unix_ms.into_series(),
            open.into_series(),
            high.into_series(),
            low.into_series(),
            close.into_series(),
            volume.into_series(),
        ])
        .unwrap()
    }

    #[tokio::test]
    async fn test_disparity_index_mean_reversion_signals() -> Result<()> {
        let df = create_test_data();

        let config = DisparityIndexMeanReversionConfig {
            period: 2,
            oversold_threshold: -5.0,
            overbought_threshold: 5.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
            max_position_size: 100.0,
        };

        let strategy = DisparityIndexMeanReversion::new(config)?;
        let signals = strategy.generate_signals(&df).await?;

        // Expect at least two signals (Long entry and Short entry)
        assert!(!signals.is_empty(), "Expected signals, got none.");

        // Check for Long Entry
        let long_entry = signals.iter().find(|s| s.side == "buy" && s.signal_type == SignalType::Entry);
        assert!(long_entry.is_some(), "Expected Long entry signal");

        // Check for Short Entry
        let short_entry = signals.iter().find(|s| s.side == "sell" && s.signal_type == SignalType::Entry);
        assert!(short_entry.is_some(), "Expected Short entry signal");

        Ok(())
    }

    #[test]
    fn test_parameter_validation() {
        let mut config = DisparityIndexMeanReversionConfig::default();

        // Invalid period
        config.period = 0;
        assert!(DisparityIndexMeanReversion::new(config.clone()).is_err());

        // Valid period
        config.period = 14;
        assert!(DisparityIndexMeanReversion::new(config.clone()).is_ok());

        // Invalid thresholds
        config.oversold_threshold = 5.0;
        config.overbought_threshold = -5.0;
        assert!(DisparityIndexMeanReversion::new(config.clone()).is_err());
    }

    #[tokio::test]
    async fn test_disparity_index_empty_data() -> Result<()> {
        let df = DataFrame::default();
        let config = DisparityIndexMeanReversionConfig::default();
        let strategy = DisparityIndexMeanReversion::new(config)?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty(), "Empty dataframe should yield no signals");
        Ok(())
    }

    #[tokio::test]
    async fn test_disparity_index_extreme_volatility() -> Result<()> {
        // Test with massive price jumps that should trigger immediate signals
        let timestamp_unix_ms = Int64Chunked::from_slice("timestamp_unix_ms", &[1000, 2000, 3000, 4000]);
        let open = Float64Chunked::from_slice("open", &[100.0, 100.0, 500.0, 10.0]);
        let high = Float64Chunked::from_slice("high", &[105.0, 105.0, 505.0, 15.0]);
        let low = Float64Chunked::from_slice("low", &[95.0, 95.0, 495.0, 5.0]);
        let close = Float64Chunked::from_slice("close", &[100.0, 100.0, 500.0, 10.0]);
        let volume = Float64Chunked::from_slice("volume", &[100.0, 100.0, 100.0, 100.0]);

        let df = DataFrame::new(vec![
            timestamp_unix_ms.into_series(),
            open.into_series(),
            high.into_series(),
            low.into_series(),
            close.into_series(),
            volume.into_series(),
        ])?;

        let config = DisparityIndexMeanReversionConfig {
            period: 2,
            oversold_threshold: -5.0,
            overbought_threshold: 5.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
            max_position_size: 100.0,
        };

        let strategy = DisparityIndexMeanReversion::new(config)?;
        let signals = strategy.generate_signals(&df).await?;

        // Extreme jump to 500 should trigger a short, drop to 10 should trigger a long.
        assert!(!signals.is_empty(), "Expected signals during extreme volatility");
        Ok(())
    }
}
