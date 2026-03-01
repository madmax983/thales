use crate::indicators::{atr, vortex};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

/// Configuration for the Vortex Breakout strategy.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct VortexBreakoutConfig {
    /// Lookback period for the Vortex Indicator (typically 14)
    pub period: usize,
    /// Multiplier for ATR-based stop loss
    pub stop_loss_atr_mult: f64,
    /// Lookback period for ATR
    pub atr_period: usize,
    /// The symbol to trade
    pub symbol: String,
}

impl StrategyConfig for VortexBreakoutConfig {}

/// Vortex Breakout Strategy
///
/// Generates signals based on the crossing of VI+ and VI- lines.
/// Buys when VI+ crosses above VI-.
/// Sells when VI+ crosses below VI-.
pub struct VortexBreakout {
    config: VortexBreakoutConfig,
}

impl VortexBreakout {
    pub fn new(config: VortexBreakoutConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for VortexBreakout {
    fn name(&self) -> &str {
        "VortexBreakout"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.period {
            return Ok(vec![]);
        }

        let (vi_plus, vi_minus) = vortex::calculate(data, self.config.period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let closes = data.column("close")?.f64()?;
        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        let vi_plus_f64 = vi_plus.f64()?;
        let vi_minus_f64 = vi_minus.f64()?;
        let atr_f64 = atr_series.f64()?;

        let mut signals = Vec::new();
        let mut in_position = false;
        let mut position_side = "";

        for i in 1..closes.len() {
            let current_close = closes.get(i).unwrap_or(0.0);
            let ts = timestamps.get(i).unwrap_or(0);

            let vi_p = vi_plus_f64.get(i);
            let vi_m = vi_minus_f64.get(i);
            let prev_vi_p = vi_plus_f64.get(i - 1);
            let prev_vi_m = vi_minus_f64.get(i - 1);
            let atr_val = atr_f64.get(i).unwrap_or(0.0);

            if let (Some(vp), Some(vm), Some(pvp), Some(pvm)) = (vi_p, vi_m, prev_vi_p, prev_vi_m) {
                // Exit condition for Long: VI+ crosses below VI-
                if in_position && position_side == "buy" && vp < vm && pvp >= pvm {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 1.0,
                        stop_loss: None,
                        take_profit: None,
                        reason: "VI+ crossed below VI-".to_string(),
                        timestamp_ms: ts,
                    });
                    in_position = false;
                }

                // Exit condition for Short: VI+ crosses above VI-
                if in_position && position_side == "sell" && vp > vm && pvp <= pvm {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 1.0,
                        stop_loss: None,
                        take_profit: None,
                        reason: "VI+ crossed above VI-".to_string(),
                        timestamp_ms: ts,
                    });
                    in_position = false;
                }

                // Entry condition for Long: VI+ crosses above VI-
                if !in_position && vp > vm && pvp <= pvm {
                    let stop_loss = current_close - (atr_val * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(), // Default hint
                        confidence: 1.0,
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: "VI+ crossed above VI-".to_string(),
                        timestamp_ms: ts,
                    });
                    in_position = true;
                    position_side = "buy";
                }

                // Entry condition for Short: VI+ crosses below VI-
                if !in_position && vp < vm && pvp >= pvm {
                    let stop_loss = current_close + (atr_val * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(), // Default hint
                        confidence: 1.0,
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: "VI+ crossed below VI-".to_string(),
                        timestamp_ms: ts,
                    });
                    in_position = true;
                    position_side = "sell";
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: VortexBreakoutConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_mock_data(highs: &[f64], lows: &[f64], closes: &[f64]) -> DataFrame {
        let n = highs.len();
        let timestamps: Vec<i64> = (0..n).map(|i| (i as i64) * 60000).collect();
        let volumes: Vec<f64> = vec![1000.0; n];
        let opens: Vec<f64> = closes.to_vec(); // Just mock opens with closes

        df!(
            "timestamp_unix_ms" => timestamps,
            "open" => opens,
            "high" => highs,
            "low" => lows,
            "close" => closes,
            "volume" => volumes
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_entry_signal_generation() {
        // Create data where VI+ crosses above VI-
        // We need a downtrend followed by a strong uptrend
        let mut highs = vec![100.0; 20];
        let mut lows = vec![90.0; 20];
        let mut closes = vec![95.0; 20];

        // Simulate uptrend breakout
        for i in 20..25 {
            highs.push(110.0 + (i as f64) * 2.0);
            lows.push(100.0 + (i as f64) * 2.0);
            closes.push(105.0 + (i as f64) * 2.0);
        }

        let df = create_mock_data(&highs, &lows, &closes);
        let config = VortexBreakoutConfig {
            period: 14,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = VortexBreakout::new(config);

        let signals = strategy.generate_signals(&df).await.unwrap();

        let buy_signals: Vec<_> = signals
            .iter()
            .filter(|s| s.side == "buy" && s.signal_type == SignalType::Entry)
            .collect();
        assert!(
            !buy_signals.is_empty(),
            "Should generate a buy entry signal on upward breakout"
        );
        assert!(
            buy_signals[0].stop_loss.is_some(),
            "Stop loss should be calculated using ATR"
        );
    }

    #[tokio::test]
    async fn test_exit_signal_generation() {
        // Create data where VI+ crosses above VI-, then crosses below
        let mut highs = vec![100.0; 20];
        let mut lows = vec![90.0; 20];
        let mut closes = vec![95.0; 20];

        // Simulate uptrend breakout
        for i in 20..25 {
            highs.push(110.0 + (i as f64) * 2.0);
            lows.push(100.0 + (i as f64) * 2.0);
            closes.push(105.0 + (i as f64) * 2.0);
        }

        // Simulate downtrend reversal. We need enough bars for VI- to cross above VI+
        for i in 25..45 {
            highs.push(90.0 - (i as f64) * 2.0);
            lows.push(80.0 - (i as f64) * 2.0);
            closes.push(85.0 - (i as f64) * 2.0);
        }

        let df = create_mock_data(&highs, &lows, &closes);
        let config = VortexBreakoutConfig {
            period: 14,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = VortexBreakout::new(config);

        let signals = strategy.generate_signals(&df).await.unwrap();

        let exit_signals: Vec<_> = signals
            .iter()
            .filter(|s| s.side == "sell" && s.signal_type == SignalType::Exit)
            .collect();
        assert!(
            !exit_signals.is_empty(),
            "Should generate an exit signal when trend reverses"
        );
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let df = create_mock_data(&[10.0], &[9.0], &[9.5]); // very short data
        let config = VortexBreakoutConfig {
            period: 14,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = VortexBreakout::new(config);

        let signals = strategy.generate_signals(&df).await.unwrap();
        assert!(
            signals.is_empty(),
            "Should handle insufficient data gracefully"
        );
    }
}
