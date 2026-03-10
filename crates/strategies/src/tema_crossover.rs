use crate::indicators::tema;
use crate::strategy::{Signal, SignalType, Strategy, StrategyType};
use anyhow::{Context, Result};
use async_trait::async_trait;
use polars::prelude::*;

pub struct TemaCrossover {
    config: TemaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TemaCrossoverConfig {
    pub fast_period: usize,
    pub medium_period: usize,
    pub slow_period: usize,
    pub stop_loss_pct: f64,
    pub symbol: String,
}

impl Default for TemaCrossoverConfig {
    fn default() -> Self {
        Self {
            fast_period: 9,
            medium_period: 21,
            slow_period: 55,
            stop_loss_pct: 0.05,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

impl TemaCrossover {
    pub fn new(config: TemaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for TemaCrossover {
    fn name(&self) -> &str {
        "TemaCrossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        let fast_tema_series = tema::calculate(data, self.config.fast_period)?;
        let medium_tema_series = tema::calculate(data, self.config.medium_period)?;
        let slow_tema_series = tema::calculate(data, self.config.slow_period)?;

        let fast_tema = fast_tema_series.f64()?;
        let medium_tema = medium_tema_series.f64()?;
        let slow_tema = slow_tema_series.f64()?;

        let close_series = data.column("close")?.f64()?;
        let timestamp_series = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let mut in_position = false;
        let mut entry_price = 0.0;
        let mut current_side = "";

        for i in 1..data.height() {
            let fast_curr = fast_tema.get(i);
            let medium_curr = medium_tema.get(i);
            let slow_curr = slow_tema.get(i);

            let fast_prev = fast_tema.get(i - 1);
            let medium_prev = medium_tema.get(i - 1);
            let slow_prev = slow_tema.get(i - 1);

            let close = close_series.get(i);
            let ts = timestamp_series.get(i);

            if fast_curr.is_none() || medium_curr.is_none() || slow_curr.is_none() ||
               fast_prev.is_none() || medium_prev.is_none() || slow_prev.is_none() ||
               close.is_none() || ts.is_none() {
                continue;
            }

            let fc = fast_curr.unwrap();
            let mc = medium_curr.unwrap();
            let sc = slow_curr.unwrap();

            let fp = fast_prev.unwrap();
            let mp = medium_prev.unwrap();
            let sp = slow_prev.unwrap();

            let c = close.unwrap();
            let t = ts.unwrap();

            let bullish_alignment_curr = fc > mc && mc > sc;
            let bullish_alignment_prev = fp > mp && mp > sp;

            let bearish_alignment_curr = fc < mc && mc < sc;
            let bearish_alignment_prev = fp < mp && mp < sp;

            if !in_position {
                // Check for long entry
                if bullish_alignment_curr && !bullish_alignment_prev {
                    in_position = true;
                    entry_price = c;
                    current_side = "buy";
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(c * (1.0 - self.config.stop_loss_pct)),
                        take_profit: None,
                        reason: "Fast TEMA crossed above Medium and Slow TEMA".to_string(),
                        timestamp_ms: t,
                    });
                }
                // Check for short entry
                else if bearish_alignment_curr && !bearish_alignment_prev {
                    in_position = true;
                    entry_price = c;
                    current_side = "sell";
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(c * (1.0 + self.config.stop_loss_pct)),
                        take_profit: None,
                        reason: "Fast TEMA crossed below Medium and Slow TEMA".to_string(),
                        timestamp_ms: t,
                    });
                }
            } else {
                let mut exit_reason = None;
                let mut is_exit = false;

                if current_side == "buy" {
                    if fc < mc {
                        is_exit = true;
                        exit_reason = Some("Fast TEMA crossed below Medium TEMA (Long Exit)");
                    } else if c <= entry_price * (1.0 - self.config.stop_loss_pct) {
                        is_exit = true;
                        exit_reason = Some("Stop Loss Hit (Long)");
                    }
                } else if current_side == "sell" {
                    if fc > mc {
                        is_exit = true;
                        exit_reason = Some("Fast TEMA crossed above Medium TEMA (Short Exit)");
                    } else if c >= entry_price * (1.0 + self.config.stop_loss_pct) {
                        is_exit = true;
                        exit_reason = Some("Stop Loss Hit (Short)");
                    }
                }

                if is_exit {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: if current_side == "buy" { "sell".to_string() } else { "buy".to_string() },
                        size_hint: "max".to_string(),
                        confidence: 1.0,
                        stop_loss: None,
                        take_profit: None,
                        reason: exit_reason.unwrap_or("Exit condition met").to_string(),
                        timestamp_ms: t,
                    });

                    in_position = false;

                    // Stop and reverse logic if valid crossover occurs on same bar
                    if current_side == "buy" && bearish_alignment_curr {
                        in_position = true;
                        entry_price = c;
                        current_side = "sell";
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(c * (1.0 + self.config.stop_loss_pct)),
                            take_profit: None,
                            reason: "Fast TEMA crossed below Medium and Slow TEMA".to_string(),
                            timestamp_ms: t,
                        });
                    } else if current_side == "sell" && bullish_alignment_curr {
                        in_position = true;
                        entry_price = c;
                        current_side = "buy";
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(c * (1.0 - self.config.stop_loss_pct)),
                            take_profit: None,
                            reason: "Fast TEMA crossed above Medium and Slow TEMA".to_string(),
                            timestamp_ms: t,
                        });
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: TemaCrossoverConfig = serde_json::from_value(params)
            .context("Failed to parse TemaCrossover parameters")?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        // Need enough data points to warmup TEMA
        let mut closes = Vec::new();
        let mut timestamps = Vec::new();
        let mut val = 100.0;
        let mut t = 1000;

        // Warmup + Flat
        for _ in 0..100 {
            closes.push(val);
            timestamps.push(t as i64);
            t += 1000;
        }

        // Bullish Uptrend
        for _ in 0..10 {
            val += 2.0;
            closes.push(val);
            timestamps.push(t as i64);
            t += 1000;
        }

        // Downtrend (Crossover below)
        for _ in 0..10 {
            val -= 2.0;
            closes.push(val);
            timestamps.push(t as i64);
            t += 1000;
        }

        df!(
            "close" => closes,
            "timestamp_unix_ms" => timestamps,
            "open" => vec![0.0; 120],
            "high" => vec![0.0; 120],
            "low" => vec![0.0; 120],
            "volume" => vec![0.0; 120]
        ).unwrap()
    }

    #[tokio::test]
    async fn test_tema_crossover_signals() {
        let df = create_test_data();
        let config = TemaCrossoverConfig {
            fast_period: 2,
            medium_period: 4,
            slow_period: 6,
            stop_loss_pct: 0.05,
            symbol: "TEST".to_string(),
        };
        let strategy = TemaCrossover::new(config);

        let signals = strategy.generate_signals(&df).await.unwrap();

        assert!(!signals.is_empty(), "Should generate signals");

        let entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry.is_some(), "Should have an entry signal");
        assert_eq!(entry.unwrap().side, "buy");

        let exit = signals.iter().find(|s| s.signal_type == SignalType::Exit);
        assert!(exit.is_some(), "Should have an exit signal");
        assert_eq!(exit.unwrap().side, "sell");
    }

    #[tokio::test]
    async fn test_empty_data() {
        let df = DataFrame::default();
        let config = TemaCrossoverConfig::default();
        let strategy = TemaCrossover::new(config);

        let signals = strategy.generate_signals(&df).await.unwrap();
        assert!(signals.is_empty());
    }

    #[tokio::test]
    async fn test_update_params() {
        let mut strategy = TemaCrossover::new(TemaCrossoverConfig::default());
        let params = serde_json::json!({
            "fast_period": 10,
            "medium_period": 20,
            "slow_period": 30,
            "stop_loss_pct": 0.1,
            "symbol": "BTCUSD"
        });

        strategy.update_params(params).await.unwrap();
        assert_eq!(strategy.config.fast_period, 10);
        assert_eq!(strategy.config.symbol, "BTCUSD");
    }
}