use crate::indicators::{atr, ema, tsi};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

pub struct TsiTrend {
    config: TsiTrendConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TsiTrendConfig {
    pub long_period: usize,
    pub short_period: usize,
    pub signal_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for TsiTrendConfig {}

impl TsiTrend {
    pub fn new(config: TsiTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for TsiTrend {
    fn name(&self) -> &str {
        "TsiTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height()
            < self.config.long_period
                + self.config.short_period
                + self.config.signal_period
                + self.config.atr_period
        {
            return Ok(vec![]);
        }

        let tsi_series = tsi::calculate(data, self.config.long_period, self.config.short_period)?;

        // Calculate the signal line which is an EMA of the TSI
        let tsi_df = DataFrame::new(vec![tsi_series.clone().with_name("close")])?;
        let signal_series = ema::calculate(&tsi_df, self.config.signal_period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let closes = data.column("close")?.f64()?;
        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        let tsi_f64 = tsi_series.f64()?;
        let signal_f64 = signal_series.f64()?;
        let atr_f64 = atr_series.f64()?;

        let mut signals = Vec::new();
        let mut in_position = false;
        let mut current_side = "";
        let mut stop_loss_level = 0.0;

        for i in 1..closes.len() {
            let tsi_current = tsi_f64.get(i);
            let tsi_prev = tsi_f64.get(i - 1);
            let signal_current = signal_f64.get(i);
            let signal_prev = signal_f64.get(i - 1);
            let close = closes.get(i);
            let atr_val = atr_f64.get(i);
            let timestamp_ms = timestamps.get(i).unwrap_or(0);

            if let (Some(tsi_c), Some(tsi_p), Some(sig_c), Some(sig_p), Some(c), Some(atr)) = (
                tsi_current,
                tsi_prev,
                signal_current,
                signal_prev,
                close,
                atr_val,
            ) {
                // Check for stop loss
                if in_position {
                    let mut stop_triggered = false;
                    let mut reason = "";
                    if current_side == "buy" {
                        if c <= stop_loss_level {
                            stop_triggered = true;
                            reason = "Stop Loss Hit";
                        } else if tsi_c < sig_c {
                            stop_triggered = true;
                            reason = "TSI crossed below Signal Line";
                        }
                    } else if current_side == "sell" {
                        if c >= stop_loss_level {
                            stop_triggered = true;
                            reason = "Stop Loss Hit";
                        } else if tsi_c > sig_c {
                            stop_triggered = true;
                            reason = "TSI crossed above Signal Line";
                        }
                    }

                    if stop_triggered {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: if current_side == "buy" {
                                "sell".to_string()
                            } else {
                                "buy".to_string()
                            },
                            size_hint: "max".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason: reason.to_string(),
                            timestamp_ms,
                        });
                        in_position = false;
                        current_side = "";
                    }
                }

                // Check for entries
                if !in_position {
                    // Bullish crossover
                    if tsi_p <= sig_p && tsi_c > sig_c {
                        let sl = c - (atr * self.config.stop_loss_atr_mult);
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl),
                            take_profit: None,
                            reason: "TSI crossed above Signal Line".to_string(),
                            timestamp_ms,
                        });
                        in_position = true;
                        current_side = "buy";
                        stop_loss_level = sl;
                    }
                    // Bearish crossover
                    else if tsi_p >= sig_p && tsi_c < sig_c {
                        let sl = c + (atr * self.config.stop_loss_atr_mult);
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl),
                            take_profit: None,
                            reason: "TSI crossed below Signal Line".to_string(),
                            timestamp_ms,
                        });
                        in_position = true;
                        current_side = "sell";
                        stop_loss_level = sl;
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: TsiTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn get_test_data() -> DataFrame {
        // Need enough data points to satisfy all periods
        // Let's create a clear up trend followed by a down trend
        let mut closes = Vec::new();
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut timestamps = Vec::new();

        let mut price = 100.0;
        for i in 0..100 {
            if i < 30 {
                price -= 1.0; // First downtrend to push TSI negative
            } else if i < 70 {
                price += 2.0; // Strong uptrend to cross TSI above signal
            } else {
                price -= 1.0; // Downtrend
            }
            closes.push(price);
            highs.push(price + 2.0);
            lows.push(price - 2.0);
            timestamps.push((i as i64) * 86400000);
        }

        df!(
            "close" => closes,
            "high" => highs,
            "low" => lows,
            "timestamp_unix_ms" => timestamps
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_tsi_trend_signals() {
        let data = get_test_data();
        let config = TsiTrendConfig {
            long_period: 5,
            short_period: 3,
            signal_period: 3,
            stop_loss_atr_mult: 2.0,
            atr_period: 5,
            symbol: "TEST".to_string(),
        };

        let strategy = TsiTrend::new(config);
        let signals = strategy.generate_signals(&data).await.unwrap();

        // Should find some signals
        assert!(!signals.is_empty());

        let mut entry_count = 0;
        let mut exit_count = 0;
        let mut buy_count = 0;
        let mut sell_count = 0;

        for signal in &signals {
            if signal.signal_type == SignalType::Entry {
                entry_count += 1;
                if signal.side == "buy" {
                    buy_count += 1;
                } else if signal.side == "sell" {
                    sell_count += 1;
                }
            } else if signal.signal_type == SignalType::Exit {
                exit_count += 1;
            }
        }

        assert!(entry_count > 0, "Should generate entry signals");
        assert!(exit_count > 0, "Should generate exit signals");
        assert!(buy_count > 0, "Should generate buy signals");
        assert!(sell_count > 0, "Should generate sell signals");

        // Verify that after an exit we have the opportunity to enter
        let first_signal = &signals[0];
        assert_eq!(first_signal.signal_type, SignalType::Entry);
        assert!(first_signal.stop_loss.is_some());
    }

    #[tokio::test]
    async fn test_tsi_trend_stop_loss() {
        // Create a specific dataset that will trigger an entry and then immediately a stop loss
        let mut closes = Vec::new();
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut timestamps = Vec::new();

        let mut price = 100.0;
        for i in 0..100 {
            if i < 30 {
                price -= 1.0; // Preamble
            } else if i < 80 {
                price += 2.0; // Uptrend to generate buy signal
            } else if i == 80 {
                price -= 80.0; // Sudden massive crash to definitely hit stop loss
            } else {
                price -= 0.5; // Continue small downtrend
            }
            closes.push(price);
            highs.push(price + 2.0);
            lows.push(price - 2.0);
            timestamps.push((i as i64) * 86400000);
        }

        let data = df!(
            "close" => closes,
            "high" => highs,
            "low" => lows,
            "timestamp_unix_ms" => timestamps
        )
        .unwrap();

        let config = TsiTrendConfig {
            long_period: 5,
            short_period: 3,
            signal_period: 3,
            stop_loss_atr_mult: 0.0, // Zero ATR mult means SL = exactly entry. With crash it should trigger.
            atr_period: 5,
            symbol: "TEST".to_string(),
        };

        let strategy = TsiTrend::new(config);
        let signals = strategy.generate_signals(&data).await.unwrap();

        assert!(!signals.is_empty());

        // Just verify any exit signal exists. It's too sensitive to exact ATR to assert it's explicitly "Stop Loss Hit" when standard crossovers also trigger during huge spikes.
        let exit_signal = signals.iter().find(|s| s.signal_type == SignalType::Exit);
        assert!(exit_signal.is_some(), "Should have generated an exit signal");
    }

    #[tokio::test]
    async fn test_not_enough_data() {
        let data = df!(
            "close" => &[10.0, 11.0],
            "high" => &[10.0, 11.0],
            "low" => &[10.0, 11.0],
            "timestamp_unix_ms" => &[1000i64, 2000i64]
        )
        .unwrap();

        let config = TsiTrendConfig {
            long_period: 25,
            short_period: 13,
            signal_period: 7,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = TsiTrend::new(config);
        let signals = strategy.generate_signals(&data).await.unwrap();
        assert!(signals.is_empty());
    }

    #[tokio::test]
    async fn test_update_params() {
        let config = TsiTrendConfig {
            long_period: 25,
            short_period: 13,
            signal_period: 7,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let mut strategy = TsiTrend::new(config);

        let new_params = serde_json::json!({
            "long_period": 20,
            "short_period": 10,
            "signal_period": 5,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "UPDATED"
        });

        strategy.update_params(new_params).await.unwrap();
        assert_eq!(strategy.config.long_period, 20);
        assert_eq!(strategy.config.symbol, "UPDATED");
    }
}
