//! The Elder-Ray Strategy
//!
//! Uses Bull and Bear power to identify trend reversals.
//!
use crate::indicators::{atr, elder_ray};
use crate::strategy::{Signal, SignalType, Strategy, StrategyType};
use anyhow::{Context, Result};
use async_trait::async_trait;
use polars::prelude::*;

pub struct ElderRay {
    config: ElderRayConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ElderRayConfig {
    pub ema_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl ElderRay {
    pub fn new(config: ElderRayConfig) -> Self {
        Self { config }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PositionSide {
    Long,
    Short,
}

#[async_trait]
impl Strategy for ElderRay {
    fn name(&self) -> &str {
        "ElderRay"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        // Calculate Indicators
        let elder_ray_out = elder_ray::calculate(data, self.config.ema_period)?;
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let ema = elder_ray_out.ema.f64()?;
        let bull_power = elder_ray_out.bull_power.f64()?;
        let bear_power = elder_ray_out.bear_power.f64()?;
        let atr = atr_series.f64()?;

        let close = data
            .column("close")
            .context("Missing 'close' column")?
            .f64()?;
        let timestamps = data
            .column("timestamp_unix_ms")
            .context("Missing 'timestamp' column")?
            .i64()?;

        let mut signals = Vec::new();
        let mut position: Option<PositionSide> = None;
        let mut stop_loss = 0.0;

        // Start from period + 1 to have previous values
        for i in 1..data.height() {
            let current_close = close.get(i);
            let ts = timestamps.get(i);

            let curr_ema = ema.get(i);
            let prev_ema = ema.get(i - 1);

            let curr_bull = bull_power.get(i);
            let prev_bull = bull_power.get(i - 1);

            let curr_bear = bear_power.get(i);
            let prev_bear = bear_power.get(i - 1);

            let curr_atr = atr.get(i);

            if let (
                Some(price),
                Some(timestamp),
                Some(ema_c),
                Some(ema_p),
                Some(bull_c),
                Some(bull_p),
                Some(bear_c),
                Some(bear_p),
                Some(atr_val),
            ) = (
                current_close,
                ts,
                curr_ema,
                prev_ema,
                curr_bull,
                prev_bull,
                curr_bear,
                prev_bear,
                curr_atr,
            ) {
                match position {
                    None => {
                        // Entry Conditions
                        // Long Entry: Bear Power < 0 and Bear Power is rising (bullish divergence), and EMA is rising
                        if bear_c < 0.0 && bear_c > bear_p && ema_c > ema_p {
                            stop_loss = price - (atr_val * self.config.stop_loss_atr_mult);
                            position = Some(PositionSide::Long);

                            signals.push(Signal {
                                symbol: self.config.symbol.clone(),
                                timestamp_ms: timestamp,
                                signal_type: SignalType::Entry,
                                side: "buy".to_string(),
                                size_hint: "100".to_string(), // Default hint
                                confidence: 1.0,
                                stop_loss: Some(stop_loss),
                                take_profit: None,
                                reason: format!(
                                    "ElderRay Long: BearPower ({:.2}) < 0 and rising. EMA rising.",
                                    bear_c
                                ),
                            });
                        }
                        // Short Entry: Bull Power > 0 and Bull Power is falling (bearish divergence), and EMA is falling
                        else if bull_c > 0.0 && bull_c < bull_p && ema_c < ema_p {
                            stop_loss = price + (atr_val * self.config.stop_loss_atr_mult);
                            position = Some(PositionSide::Short);

                            signals.push(Signal {
                                symbol: self.config.symbol.clone(),
                                timestamp_ms: timestamp,
                                signal_type: SignalType::Entry,
                                side: "sell".to_string(),
                                size_hint: "100".to_string(),
                                confidence: 1.0,
                                stop_loss: Some(stop_loss),
                                take_profit: None,
                                reason: format!(
                                    "ElderRay Short: BullPower ({:.2}) > 0 and falling. EMA falling.",
                                    bull_c
                                ),
                            });
                        }
                    }
                    Some(PositionSide::Long) => {
                        // Exit Conditions
                        // Long Exit: Bear Power starts falling or EMA turns down OR Price <= Stop Loss
                        if price <= stop_loss {
                            position = None;
                            signals.push(Signal {
                                symbol: self.config.symbol.clone(),
                                timestamp_ms: timestamp,
                                signal_type: SignalType::Exit,
                                side: "sell".to_string(),
                                size_hint: "max".to_string(),
                                confidence: 1.0,
                                stop_loss: None,
                                take_profit: None,
                                reason: format!(
                                    "ElderRay Long Exit: Stop Loss hit at {}",
                                    stop_loss
                                ),
                            });
                        } else if bear_c < bear_p || ema_c < ema_p {
                            position = None;
                            signals.push(Signal {
                                symbol: self.config.symbol.clone(),
                                timestamp_ms: timestamp,
                                signal_type: SignalType::Exit,
                                side: "sell".to_string(),
                                size_hint: "max".to_string(),
                                confidence: 1.0,
                                stop_loss: None,
                                take_profit: None,
                                reason: "ElderRay Long Exit: Bear Power falling or EMA turned down"
                                    .to_string(),
                            });
                        }
                    }
                    Some(PositionSide::Short) => {
                        // Exit Conditions
                        // Short Exit: Bull Power starts rising or EMA turns up OR Price >= Stop Loss
                        if price >= stop_loss {
                            position = None;
                            signals.push(Signal {
                                symbol: self.config.symbol.clone(),
                                timestamp_ms: timestamp,
                                signal_type: SignalType::Exit,
                                side: "buy".to_string(),
                                size_hint: "max".to_string(),
                                confidence: 1.0,
                                stop_loss: None,
                                take_profit: None,
                                reason: format!(
                                    "ElderRay Short Exit: Stop Loss hit at {}",
                                    stop_loss
                                ),
                            });
                        } else if bull_c > bull_p || ema_c > ema_p {
                            position = None;
                            signals.push(Signal {
                                symbol: self.config.symbol.clone(),
                                timestamp_ms: timestamp,
                                signal_type: SignalType::Exit,
                                side: "buy".to_string(),
                                size_hint: "max".to_string(),
                                confidence: 1.0,
                                stop_loss: None,
                                take_profit: None,
                                reason: "ElderRay Short Exit: Bull Power rising or EMA turned up"
                                    .to_string(),
                            });
                        }
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: ElderRayConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_elder_ray_long_signals() -> Result<()> {
        let strategy = ElderRay::new(ElderRayConfig {
            ema_period: 3,
            stop_loss_atr_mult: 2.0,
            atr_period: 3,
            symbol: "BTCUSD".to_string(),
        });

        // Construct mock data
        let df = df!(
            "timestamp_unix_ms" => &[1i64, 2, 3, 4, 5, 6],
            "open" => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "high" => &[11.0, 11.0, 11.0, 12.0, 13.0, 14.0], // Constant then rising
            "low" => &[9.0, 9.0, 9.0, 10.0, 11.0, 9.0],      // Bear Power < 0 and rising, then falling
            "close" => &[10.0, 10.0, 10.0, 11.0, 12.0, 11.0]  // EMA rising, then falling
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect long entry when EMA is rising, Bear Power is < 0 but rising
        let entry_sig = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "buy");
        assert!(entry_sig.is_some());

        // Expect long exit when EMA drops or Bear Power falls
        let exit_sig = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Exit && s.side == "sell");
        assert!(exit_sig.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_elder_ray_short_signals() -> Result<()> {
        let strategy = ElderRay::new(ElderRayConfig {
            ema_period: 3,
            stop_loss_atr_mult: 2.0,
            atr_period: 3,
            symbol: "BTCUSD".to_string(),
        });

        let df2 = df!(
            "timestamp_unix_ms" => &[1i64, 2, 3, 4, 5, 6],
            "open" => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "high" => &[11.0, 11.0, 11.0, 10.5, 9.5, 11.0], // Need Bull Power > 0 and falling
            "low" => &[9.0, 9.0, 9.0, 8.0, 7.0, 9.0],
            "close" => &[10.0, 10.0, 10.0, 9.0, 8.0, 10.0] // EMA falling
        )?;

        let signals = strategy.generate_signals(&df2).await?;

        // Expect short entry when EMA is falling, Bull Power > 0 and falling
        let entry_sig = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "sell");
        assert!(entry_sig.is_some());

        // Expect short exit when EMA rises or Bull power rises
        let exit_sig = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Exit && s.side == "buy");
        assert!(exit_sig.is_some());

        Ok(())
    }
}
