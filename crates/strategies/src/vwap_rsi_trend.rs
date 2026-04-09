use crate::indicators::{rsi, vwap};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VwapRsiTrendConfig {
    pub rsi_period: usize,
    pub rsi_threshold: f64,
    pub symbol: String,
}

impl Default for VwapRsiTrendConfig {
    fn default() -> Self {
        Self {
            rsi_period: 14,
            rsi_threshold: 50.0,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for VwapRsiTrendConfig {}

pub struct VwapRsiTrend {
    config: VwapRsiTrendConfig,
}

impl VwapRsiTrend {
    pub fn new(config: VwapRsiTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for VwapRsiTrend {
    fn name(&self) -> &str {
        "VwapRsiTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.rsi_period + 1 {
            return Ok(vec![]);
        }

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let close_series = data.column("close")?.f64()?;

        let vwap_series = vwap::calculate(data)?;
        let vwap_arr = vwap_series.f64()?;

        let rsi_series = rsi::calculate(data, self.config.rsi_period)?;
        let rsi_arr = rsi_series.f64()?;

        let vwap_vec: Vec<Option<f64>> = vwap_arr.into_iter().collect();
        let rsi_vec: Vec<Option<f64>> = rsi_arr.into_iter().collect();
        let close_vec: Vec<Option<f64>> = close_series.into_iter().collect();

        let mut signals = Vec::new();
        let len = data.height();

        for i in self.config.rsi_period..len {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price = close_vec.get(i).copied().flatten().unwrap_or(0.0);
            let prev_price = close_vec.get(i - 1).copied().flatten().unwrap_or(0.0);

            let curr_vwap = vwap_vec.get(i).copied().flatten();
            let prev_vwap = vwap_vec.get(i - 1).copied().flatten();
            let curr_rsi = rsi_vec.get(i).copied().flatten();

            if let (Some(vwap), Some(p_vwap), Some(rsi_val)) = (curr_vwap, prev_vwap, curr_rsi) {
                // Entry Long: Price crosses above VWAP and RSI > threshold
                if prev_price <= p_vwap && price > vwap && rsi_val > self.config.rsi_threshold {
                    // Simple stop loss 2% below, take profit 4% above for demo purposes (can be enhanced with ATR)
                    let sl = price * 0.98;
                    let tp = price * 1.04;

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: Some(tp),
                        reason: "VWAP RSI Golden Cross".to_string(),
                        timestamp_ms: timestamp,
                    });
                }

                // Exit Long: Price crosses below VWAP
                if prev_price >= p_vwap && price < vwap {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "VWAP RSI Death Cross".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: VwapRsiTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;


    #[tokio::test]
    async fn test_vwap_rsi_trend_empty_data() -> Result<()> {
        let config = VwapRsiTrendConfig::default();
        let strategy = VwapRsiTrend::new(config);

        // Use empty dataframe for all required columns
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
    async fn test_vwap_rsi_trend_entry_and_exit() -> Result<()> {
        let config = VwapRsiTrendConfig {
            rsi_period: 2,
            rsi_threshold: 50.0,
            symbol: "TEST".to_string(),
        };
        let strategy = VwapRsiTrend::new(config);

        let timestamps: Vec<i64> = (0..20).map(|i| 1000 + i as i64 * 1000).collect();

        let mut closes = vec![100.0; 20];
        let volumes = vec![1000.0; 20];

        // Uptrend starting to trigger RSI > 50 and price > VWAP
        closes[0] = 100.0;
        closes[1] = 100.0;
        closes[2] = 105.0; // Price crosses above VWAP and RSI spikes
        closes[3] = 106.0;

        // Downtrend to trigger price < VWAP
        closes[4] = 95.0; // Price drops below VWAP
        closes[5] = 90.0;

        let highs = closes.iter().map(|c| c + 2.0).collect::<Vec<_>>();
        let lows = closes.iter().map(|c| c - 2.0).collect::<Vec<_>>();

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
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();

        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit && s.side == "sell")
            .collect();

        assert!(!entries.is_empty(), "Should generate an entry signal");
        assert!(!exits.is_empty(), "Should generate an exit signal");
        assert!(entries[0].reason.contains("Golden Cross"));
        assert!(exits[0].reason.contains("Death Cross"));
        assert!(entries[0].stop_loss.is_some());
        assert!(entries[0].take_profit.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_vwap_rsi_trend_parameter_validation() -> Result<()> {
        let mut strategy = VwapRsiTrend::new(VwapRsiTrendConfig::default());

        assert_eq!(strategy.name(), "VwapRsiTrend");
        assert_eq!(strategy.strategy_type(), StrategyType::TrendFollowing);

        let new_params = serde_json::json!({
            "rsi_period": 21,
            "rsi_threshold": 60.0,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.rsi_period, 21);
        assert_eq!(strategy.config.rsi_threshold, 60.0);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
