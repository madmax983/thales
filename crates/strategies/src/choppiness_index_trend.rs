use crate::indicators::{atr, choppiness_index, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoppinessIndexTrendConfig {
    pub chop_period: usize,
    pub chop_threshold: f64,
    pub sma_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for ChoppinessIndexTrendConfig {}

pub struct ChoppinessIndexTrend {
    config: ChoppinessIndexTrendConfig,
}

impl ChoppinessIndexTrend {
    pub fn new(config: ChoppinessIndexTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for ChoppinessIndexTrend {
    fn name(&self) -> &str {
        "ChoppinessIndexTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate CHOP
        let chop_series = choppiness_index::calculate(data, self.config.chop_period)?;
        let chop_arr = chop_series.f64()?;

        // Calculate SMA for trend direction
        let sma_series = sma::calculate(data, self.config.sma_period)?;
        let sma_arr = sma_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);
            let prev_price_opt = close_arr.get(i - 1).and_then(Decimal::from_f64_retain);
            let chop_val_opt = chop_arr.get(i);
            let prev_chop_val_opt = chop_arr.get(i - 1);
            let sma_val_opt = sma_arr.get(i).and_then(Decimal::from_f64_retain);
            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (
                Some(price),
                Some(_prev_price),
                Some(chop_val),
                Some(prev_chop_val),
                Some(sma_val),
                Some(atr_val),
            ) = (
                price_opt,
                prev_price_opt,
                chop_val_opt,
                prev_chop_val_opt,
                sma_val_opt,
                atr_opt,
            ) {
                // Entry condition: Market transitions from choppy to trending
                // (CHOP crosses below threshold) AND price direction relative to SMA confirms trend
                if prev_chop_val >= self.config.chop_threshold && chop_val < self.config.chop_threshold {
                    if price > sma_val {
                        // Long Entry
                        let sl_dist = atr_val * stop_loss_mult_dec;
                        let sl = price - sl_dist;

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: None,
                            reason: format!(
                                "CHOP dropped below threshold ({:.2} < {:.2}) in Uptrend (Price > SMA)",
                                chop_val, self.config.chop_threshold
                            ),
                            timestamp_ms: timestamp,
                        });
                    } else if price < sma_val {
                        // Short Entry
                        let sl_dist = atr_val * stop_loss_mult_dec;
                        let sl = price + sl_dist;

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: None,
                            reason: format!(
                                "CHOP dropped below threshold ({:.2} < {:.2}) in Downtrend (Price < SMA)",
                                chop_val, self.config.chop_threshold
                            ),
                            timestamp_ms: timestamp,
                        });
                    }
                }

                // Exit condition: Market transitions from trending to choppy
                // (CHOP crosses above threshold)
                if prev_chop_val < self.config.chop_threshold && chop_val >= self.config.chop_threshold {
                    // Because we do not track state here, we emit exits for both sides
                    // to ensure any open position is closed. The execution engine handles
                    // ignoring exits for positions we do not hold.

                    // Exit Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // sell to close long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "CHOP rose above threshold ({:.2} >= {:.2}), Market returning to choppy",
                            chop_val, self.config.chop_threshold
                        ),
                        timestamp_ms: timestamp,
                    });

                    // Exit Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // buy to close short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "CHOP rose above threshold ({:.2} >= {:.2}), Market returning to choppy",
                            chop_val, self.config.chop_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: ChoppinessIndexTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    // Helper to run CHOP calculation mentally (approximated for test)
    // CHOP formula is 100 * log10( Sum(TR, n) / (Max(H, n) - Min(L, n)) ) / log10(n)
    // We will just craft df data such that CHOP values behave predictably.
    // Instead of precise CHOP values, we use mock-like behavior where we check the indicator output directly
    // Wait, we don't mock indicators here, we use the real ones. So we must provide data that triggers it.
    // An easier way is to test the logic directly if possible, or provide a dataset that definitely produces CHOP > 61.8 and CHOP < 61.8.
    // Let's create a DataFrame with known CHOP-like properties or just use edge cases and realistic data.

    #[tokio::test]
    async fn test_choppiness_index_trend_empty_data() {
        let df = DataFrame::default();
        let strategy = ChoppinessIndexTrend::new(ChoppinessIndexTrendConfig {
            chop_period: 14,
            chop_threshold: 61.8,
            sma_period: 20,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });
        let result = strategy.generate_signals(&df).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_choppiness_index_trend_parameter_update() -> Result<()> {
        let mut strategy = ChoppinessIndexTrend::new(ChoppinessIndexTrendConfig {
            chop_period: 14,
            chop_threshold: 61.8,
            sma_period: 20,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "chop_period": 20,
            "chop_threshold": 50.0,
            "sma_period": 50,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 20,
            "symbol": "NEW_TEST"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.chop_period, 20);
        assert_eq!(strategy.config.chop_threshold, 50.0);
        assert_eq!(strategy.config.sma_period, 50);
        assert_eq!(strategy.config.symbol, "NEW_TEST");

        Ok(())
    }

    #[tokio::test]
    async fn test_choppiness_index_trend_logic_with_fake_data() -> Result<()> {
        // We will generate a synthetic dataset where we know the trend and choppiness.
        // For testing signal logic safely, we can just make sure the strategy doesn't panic on a valid df
        // and generates *some* signals or empty vec. Since calculating exact CHOP by hand is hard,
        // we'll provide a clear trend down and a clear trend up.

        let mut timestamp = Vec::new();
        let mut high = Vec::new();
        let mut low = Vec::new();
        let mut close = Vec::new();
        let mut volume = Vec::new();

        let mut current_price = 100.0;

        // Phase 1: Choppy sideways market (CHOP should rise)
        for i in 0..20 {
            timestamp.push((i as i64) * 1000);
            high.push(current_price + 2.0);
            low.push(current_price - 2.0);
            close.push(current_price + if i % 2 == 0 { 1.0 } else { -1.0 });
            volume.push(100.0);
        }

        // Phase 2: Strong Uptrend (CHOP should fall)
        for i in 20..40 {
            timestamp.push((i as i64) * 1000);
            current_price += 2.0;
            high.push(current_price + 1.0);
            low.push(current_price - 1.0);
            close.push(current_price);
            volume.push(100.0);
        }

        // Phase 3: Consolidation / Choppy (CHOP should rise)
        for i in 40..60 {
            timestamp.push((i as i64) * 1000);
            high.push(current_price + 2.0);
            low.push(current_price - 2.0);
            close.push(current_price + if i % 2 == 0 { 1.0 } else { -1.0 });
            volume.push(100.0);
        }

        let df = df!(
            "timestamp_unix_ms" => timestamp,
            "high" => high,
            "low" => low,
            "close" => close,
            "volume" => volume
        )?;

        let strategy = ChoppinessIndexTrend::new(ChoppinessIndexTrendConfig {
            chop_period: 14,
            chop_threshold: 60.0,
            sma_period: 10,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let signals = strategy.generate_signals(&df).await?;

        // We should see at least an Entry (buy) during the uptrend and an Exit when it consolidates
        let mut has_buy_entry = false;
        let mut has_exit = false;

        for s in signals {
            if s.signal_type == SignalType::Entry && s.side == "buy" {
                has_buy_entry = true;
            }
            if s.signal_type == SignalType::Exit {
                has_exit = true;
            }
        }

        assert!(has_buy_entry, "Expected a Long Entry signal during the strong uptrend phase");
        assert!(has_exit, "Expected an Exit signal during the consolidation phase");

        Ok(())
    }
}
