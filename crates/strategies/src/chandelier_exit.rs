use crate::indicators::atr;
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChandelierExitConfig {
    pub period: usize,
    pub atr_mult: f64,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}

impl StrategyConfig for ChandelierExitConfig {}

pub struct ChandelierExit {
    config: ChandelierExitConfig,
}

impl ChandelierExit {
    pub fn new(config: ChandelierExitConfig) -> Self {
        Self { config }
    }
}

// O(N) rolling max using VecDeque
fn rolling_max_opt(data: &[Option<f64>], window: usize) -> Vec<Option<f64>> {
    let mut result = Vec::with_capacity(data.len());
    let mut deque: VecDeque<usize> = VecDeque::new();

    for i in 0..data.len() {
        if !deque.is_empty() && deque.front().unwrap() + window <= i {
            deque.pop_front();
        }

        if let Some(val) = data[i] {
            while !deque.is_empty() {
                if let Some(back_idx) = deque.back() {
                    if let Some(back_val) = data[*back_idx] {
                        if val >= back_val {
                            deque.pop_back();
                            continue;
                        }
                    } else {
                        deque.pop_back();
                        continue;
                    }
                }
                break;
            }
            deque.push_back(i);
        }

        if i < window - 1 {
            result.push(None);
        } else {
            if let Some(front_idx) = deque.front() {
                result.push(data[*front_idx]);
            } else {
                result.push(None);
            }
        }
    }
    result
}

// O(N) rolling min using VecDeque
fn rolling_min_opt(data: &[Option<f64>], window: usize) -> Vec<Option<f64>> {
    let mut result = Vec::with_capacity(data.len());
    let mut deque: VecDeque<usize> = VecDeque::new();

    for i in 0..data.len() {
        if !deque.is_empty() && deque.front().unwrap() + window <= i {
            deque.pop_front();
        }

        if let Some(val) = data[i] {
            while !deque.is_empty() {
                if let Some(back_idx) = deque.back() {
                    if let Some(back_val) = data[*back_idx] {
                        if val <= back_val {
                            deque.pop_back();
                            continue;
                        }
                    } else {
                        deque.pop_back();
                        continue;
                    }
                }
                break;
            }
            deque.push_back(i);
        }

        if i < window - 1 {
            result.push(None);
        } else {
            if let Some(front_idx) = deque.front() {
                result.push(data[*front_idx]);
            } else {
                result.push(None);
            }
        }
    }
    result
}

#[async_trait]
impl Strategy for ChandelierExit {
    fn name(&self) -> &str {
        "ChandelierExit"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let period = self.config.period;
        if data.height() <= period {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let high_series = data.column("high")?.clone();
        let high_arr = high_series.f64()?;

        let low_series = data.column("low")?.clone();
        let low_arr = low_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        // Calculate rolling highest high and lowest low
        let high_vec: Vec<Option<f64>> = high_arr.into_iter().collect();
        let low_vec: Vec<Option<f64>> = low_arr.into_iter().collect();

        let highest_high = rolling_max_opt(&high_vec, period);
        let lowest_low = rolling_min_opt(&low_vec, period);

        let mut signals = Vec::new();
        let mut trend = 0; // 1 for long, -1 for short, 0 for initial
        let atr_mult = self.config.atr_mult;
        let sl_mult = Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in period..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let price_opt = close_arr.get(i);
            let prev_price_opt = close_arr.get(i - 1);
            let atr_opt = atr_arr.get(i);
            let prev_atr_opt = atr_arr.get(i - 1);

            let hh = highest_high[i];
            let ll = lowest_low[i];

            if let (Some(price), Some(prev_price), Some(atr_val), Some(prev_atr_val), Some(h_high), Some(l_low)) =
                (price_opt, prev_price_opt, atr_opt, prev_atr_opt, hh, ll)
            {
                let ce_long = h_high - prev_atr_val * atr_mult;
                let ce_short = l_low + prev_atr_val * atr_mult;

                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Check for trend changes based on close vs Chandelier Exit
                // Typically: Close < CE Long means downtrend. Close > CE Short means uptrend.

                if trend != 1 && prev_price <= ce_short && price > ce_short {
                    // Trend flipped to Long
                    trend = 1;

                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short (Buy to cover)
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("Chandelier Exit Uptrend: Close {:.2} > CE Short {:.2}", price, ce_short),
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
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!("Chandelier Exit Uptrend: Close {:.2} > CE Short {:.2}", price, ce_short),
                        timestamp_ms: timestamp,
                    });
                } else if trend != -1 && prev_price >= ce_long && price < ce_long {
                    // Trend flipped to Short
                    trend = -1;

                    // Exit any Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("Chandelier Exit Downtrend: Close {:.2} < CE Long {:.2}", price, ce_long),
                        timestamp_ms: timestamp,
                    });

                    // Enter Short
                    let sl = price_dec + (atr_dec * sl_mult);
                    let risk = sl - price_dec;
                    let tp = price_dec - (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Entry Short
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!("Chandelier Exit Downtrend: Close {:.2} < CE Long {:.2}", price, ce_long),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: ChandelierExitConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_chandelier_exit_signals() -> Result<()> {
        let config = ChandelierExitConfig {
            period: 2,
            atr_mult: 1.0,
            atr_period: 2,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = ChandelierExit::new(config);

        // We need data that establishes ATR and HighestHigh/LowestLow.
        // atr_period is 2, so ATR will be calculated starting at index 1.
        // highest/lowest over period 2 starting at index 1.

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "open"  => &[10.0, 10.0, 10.0, 20.0, 5.0],
            "high"  => &[10.5, 11.0, 10.5, 21.0, 6.0],
            "low"   => &[9.5,  9.0,  9.5,  19.0, 4.0],
            "close" => &[10.0, 10.0, 10.0, 20.0, 5.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // At index 3 (4000ms), price jumps to 20.
        // It should cross above CE Short and trigger an entry.
        let entries_long: Vec<_> = signals.iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();
        assert!(entries_long.len() > 0);

        // At index 4 (5000ms), price drops to 5.
        // It should cross below CE Long and trigger a short entry.
        let entries_short: Vec<_> = signals.iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "sell")
            .collect();
        assert!(entries_short.len() > 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let config = ChandelierExitConfig {
            period: 10,
            atr_mult: 2.0,
            atr_period: 14,
            stop_loss_atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = ChandelierExit::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000],
            "high" => &[100.0, 102.0],
            "low" => &[90.0, 92.0],
            "close" => &[95.0, 96.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;
        assert!(signals.is_empty());

        Ok(())
    }
}
