use crate::indicators::{atr, macd};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacdCrossoverConfig {
    pub fast_period: usize,
    pub slow_period: usize,
    pub signal_period: usize,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}

impl Default for MacdCrossoverConfig {
    fn default() -> Self {
        Self {
            fast_period: 12,
            slow_period: 26,
            signal_period: 9,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl StrategyConfig for MacdCrossoverConfig {}

pub struct MacdCrossover {
    config: MacdCrossoverConfig,
}

impl MacdCrossover {
    pub fn new(config: MacdCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for MacdCrossover {
    fn name(&self) -> &str {
        "MacdCrossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let (macd_line, macd_signal, _macd_hist) = macd::calculate(
            data,
            self.config.fast_period,
            self.config.slow_period,
            self.config.signal_period,
        )?;

        let macd_line_arr = macd_line.f64()?;
        let macd_signal_arr = macd_signal.f64()?;

        // Calculate ATR for dynamic stop loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);

            // Ensure we have MACD values
            let macd_curr_opt = macd_line_arr.get(i).and_then(Decimal::from_f64_retain);
            let sig_curr_opt = macd_signal_arr.get(i).and_then(Decimal::from_f64_retain);
            let macd_prev_opt = macd_line_arr.get(i - 1).and_then(Decimal::from_f64_retain);
            let sig_prev_opt = macd_signal_arr.get(i - 1).and_then(Decimal::from_f64_retain);

            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(macd_c), Some(sig_c), Some(macd_p), Some(sig_p), Some(price)) = (
                macd_curr_opt,
                sig_curr_opt,
                macd_prev_opt,
                sig_prev_opt,
                price_opt,
            ) {
                // Bearish Crossover (Exit)
                if macd_c < sig_c && macd_p >= sig_p {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(), // Close position
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bearish MACD Crossover: Line {} < Signal {}",
                            macd_c.round_dp(2),
                            sig_c.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Bullish Crossover (Entry)
                if macd_c > sig_c && macd_p <= sig_p {
                    // Use ATR-based Stop Loss
                    let sl = if let Some(atr_val) = atr_opt {
                        price - (atr_val * atr_mult_dec)
                    } else {
                        // Fallback 5% stop loss if ATR is not ready
                        let five_pct = Decimal::new(5, 2); // 0.05
                        price * (Decimal::ONE - five_pct)
                    };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "Bullish MACD Crossover: Line {} > Signal {}",
                            macd_c.round_dp(2),
                            sig_c.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: MacdCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_macd_crossover_signals() -> Result<()> {
        let config = MacdCrossoverConfig {
            fast_period: 2,
            slow_period: 4,
            signal_period: 2,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = MacdCrossover::new(config);

        // Warm up MACD and trigger crossover
        let mut closes = vec![10.0; 10]; // 10 periods flat
        closes.extend_from_slice(&[15.0, 20.0, 25.0, 10.0, 5.0, 2.0]);

        let n = closes.len();
        let mut highs = Vec::with_capacity(n);
        let mut lows = Vec::with_capacity(n);
        let mut timestamps = Vec::with_capacity(n);

        for (i, &c) in closes.iter().enumerate() {
            highs.push(c + 0.5);
            lows.push(c - 0.5);
            timestamps.push((i as i64 + 1) * 1000);
        }

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect at least one entry and one exit
        let has_entry = signals.iter().any(|s| s.signal_type == SignalType::Entry);
        let has_exit = signals.iter().any(|s| s.signal_type == SignalType::Exit);

        assert!(has_entry, "Should generate entry signal");
        assert!(has_exit, "Should generate exit signal");

        // Validate entry properties
        if let Some(entry) = signals.iter().find(|s| s.signal_type == SignalType::Entry) {
            assert_eq!(entry.side, "buy");
            assert!(entry.stop_loss.is_some());
            let sl = entry.stop_loss.unwrap();
            assert!(sl > 0.0);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = MacdCrossover::new(MacdCrossoverConfig::default());

        let new_params = serde_json::json!({
            "fast_period": 5,
            "slow_period": 10,
            "signal_period": 5,
            "atr_period": 10,
            "stop_loss_atr_mult": 1.5,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.fast_period, 5);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
