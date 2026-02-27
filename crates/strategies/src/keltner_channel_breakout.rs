use crate::indicators::{atr, ema};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeltnerChannelBreakoutConfig {
    pub ema_period: usize,
    pub atr_period: usize,
    pub atr_multiplier: f64,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}

impl StrategyConfig for KeltnerChannelBreakoutConfig {}

pub struct KeltnerChannelBreakout {
    config: KeltnerChannelBreakoutConfig,
}

impl KeltnerChannelBreakout {
    pub fn new(config: KeltnerChannelBreakoutConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for KeltnerChannelBreakout {
    fn name(&self) -> &str {
        "KeltnerChannelBreakout"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Breakout
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let max_period = std::cmp::max(self.config.ema_period, self.config.atr_period);
        if data.height() < max_period + 1 {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?.f64()?;
        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Indicators
        let ema_series = ema::calculate(data, self.config.ema_period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let ema_arr = ema_series.f64()?;
        let atr_arr = atr_series.f64()?;

        let atr_mult = self.config.atr_multiplier;
        let stop_loss_mult = self.config.stop_loss_atr_mult;

        let ema_vec: Vec<Option<f64>> = ema_arr.into_iter().map(|v| v).collect();
        let atr_vec: Vec<Option<f64>> = atr_arr.into_iter().map(|v| v).collect();

        let mut signals = Vec::new();
        let len = data.height();

        for i in max_period..len {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_series.get(i);

            // Current bar values
            let ema_val_opt = ema_vec.get(i).copied().flatten();
            let atr_val_opt = atr_vec.get(i).copied().flatten();

            if let (Some(price), Some(ema_val), Some(atr_val)) =
                (price_opt, ema_val_opt, atr_val_opt)
            {
                let upper = ema_val + (atr_mult * atr_val);
                let lower = ema_val - (atr_mult * atr_val);

                // Entry Long (Breakout Upper)
                if price > upper {
                    let sl = price - (stop_loss_mult * atr_val);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: None,
                        reason: format!(
                            "Keltner Breakout: Close {:.2} > Upper {:.2}",
                            price, upper
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Exit Long (Close below EMA)
                if price < ema_val {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Close Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("Trend Change: Close {:.2} < EMA {:.2}", price, ema_val),
                        timestamp_ms: timestamp,
                    });
                }

                // Entry Short (Breakout Lower)
                if price < lower {
                    let sl = price + (stop_loss_mult * atr_val);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Short Entry
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: None,
                        reason: format!(
                            "Keltner Breakdown: Close {:.2} < Lower {:.2}",
                            price, lower
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Exit Short (Close above EMA)
                if price > ema_val {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Close Short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("Trend Change: Close {:.2} > EMA {:.2}", price, ema_val),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: KeltnerChannelBreakoutConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_keltner_entry_long() -> Result<()> {
        let config = KeltnerChannelBreakoutConfig {
            ema_period: 20,
            atr_period: 10,
            atr_multiplier: 2.0,
            stop_loss_atr_mult: 1.0,
            symbol: "TEST".to_string(),
        };
        let strategy = KeltnerChannelBreakout::new(config);

        // Generate data that triggers a breakout
        let mut closes = vec![100.0; 30]; // Stable price
                                          // At index 29 (last one), spike up to 120.
                                          // EMA(20) ~ 100. ATR(10) ~ 0 (if flat).
                                          // Let's make ATR non-zero by having previous volatility.
                                          // Or just trust the indicator logic handles flat line (ATR=0).
                                          // If ATR=0, Upper=EMA.
        closes[29] = 101.0;

        let timestamps: Vec<i64> = (0..30).map(|i| 1000 + i as i64 * 1000).collect();
        let highs = closes.iter().map(|c| c + 1.0).collect::<Vec<_>>();
        let lows = closes.iter().map(|c| c - 1.0).collect::<Vec<_>>();

        let df = df!(
            "timestamp_unix_ms" => timestamps.clone(),
            "open" => &closes,
            "high" => &highs,
            "low" => &lows,
            "close" => &closes,
            "volume" => vec![1000.0; 30]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // With stable price 100, EMA=100.
        // High 101, Low 99. TR = 2. ATR ~ 2.
        // Upper = 100 + 2*2 = 104.
        // Close 101 < 104. No signal.

        // Let's create a bigger breakout.
        let mut closes_2 = vec![100.0; 30];
        closes_2[29] = 110.0; // Breakout > 104
        let highs_2 = closes_2.iter().map(|c| c + 1.0).collect::<Vec<_>>();
        let lows_2 = closes_2.iter().map(|c| c - 1.0).collect::<Vec<_>>();

        let df_2 = df!(
            "timestamp_unix_ms" => timestamps, // reuse
            "open" => &closes_2,
            "high" => &highs_2,
            "low" => &lows_2,
            "close" => &closes_2,
            "volume" => vec![1000.0; 30]
        )?;

        let signals_2 = strategy.generate_signals(&df_2).await?;
        let entries: Vec<_> = signals_2
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();

        assert_eq!(entries.len(), 1);
        assert!(entries[0].reason.contains("Keltner Breakout"));

        Ok(())
    }

    #[tokio::test]
    async fn test_keltner_exit_long() -> Result<()> {
        let config = KeltnerChannelBreakoutConfig {
            ema_period: 20,
            atr_period: 10,
            atr_multiplier: 2.0,
            stop_loss_atr_mult: 1.0,
            symbol: "TEST".to_string(),
        };
        let strategy = KeltnerChannelBreakout::new(config);

        // Price drops below EMA
        let mut closes = vec![100.0; 30];
        closes[29] = 90.0; // Drop below EMA ~100

        let timestamps: Vec<i64> = (0..30).map(|i| 1000 + i as i64 * 1000).collect();
        let highs = closes.iter().map(|c| c + 1.0).collect::<Vec<_>>();
        let lows = closes.iter().map(|c| c - 1.0).collect::<Vec<_>>();

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "open" => &closes,
            "high" => &highs,
            "low" => &lows,
            "close" => &closes,
            "volume" => vec![1000.0; 30]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit && s.side == "sell")
            .collect();
        assert_eq!(exits.len(), 1);
        assert!(exits[0].reason.contains("Trend Change"));

        Ok(())
    }
}
