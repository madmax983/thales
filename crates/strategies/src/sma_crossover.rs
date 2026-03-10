use crate::indicators::{atr, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SmaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for SmaCrossoverConfig {}

impl Default for SmaCrossoverConfig {
    fn default() -> Self {
        Self {
            short_period: 9,
            long_period: 21,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "BTCUSD".to_string(),
        }
    }
}

pub struct SmaCrossover {
    config: SmaCrossoverConfig,
}

impl SmaCrossover {
    pub fn new(config: SmaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for SmaCrossover {
    fn name(&self) -> &str {
        "SmaCrossover"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.long_period {
            return Ok(vec![]);
        }

        let short_sma = sma::calculate(data, self.config.short_period)?;
        let long_sma = sma::calculate(data, self.config.long_period)?;
        let atr = atr::calculate(data, self.config.atr_period)?;

        let short_sma_f64 = short_sma.f64()?;
        let long_sma_f64 = long_sma.f64()?;
        let atr_f64 = atr.f64()?;

        let close = data.column("close")?.cast(&DataType::Float64)?;
        let close_f64 = close.f64()?;

        let timestamps = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let mut in_position = false;
        let mut position_side = "";

        for i in 1..data.height() {
            let current_short = short_sma_f64.get(i);
            let prev_short = short_sma_f64.get(i - 1);
            let current_long = long_sma_f64.get(i);
            let prev_long = long_sma_f64.get(i - 1);
            let current_close = close_f64.get(i);
            let current_atr = atr_f64.get(i);
            let current_ts = timestamps.get(i).unwrap_or(0);

            if let (
                Some(curr_s),
                Some(prev_s),
                Some(curr_l),
                Some(prev_l),
                Some(close_val),
                Some(atr_val),
            ) = (
                current_short,
                prev_short,
                current_long,
                prev_long,
                current_close,
                current_atr,
            ) {
                // Long Entry: Short crosses above Long
                if prev_s <= prev_l && curr_s > curr_l {
                    if in_position && position_side == "short" {
                        // Exit short first
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason: "Short SMA crossed above Long SMA".to_string(),
                            timestamp_ms: current_ts,
                        });
                    }

                    if !in_position || position_side == "short" {
                        in_position = true;
                        position_side = "long";
                        let stop_loss = close_val - (atr_val * self.config.stop_loss_atr_mult);
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 1.0,
                            stop_loss: Some(stop_loss),
                            take_profit: None,
                            reason: "Short SMA crossed above Long SMA".to_string(),
                            timestamp_ms: current_ts,
                        });
                    }
                }
                // Short Entry: Short crosses below Long
                else if prev_s >= prev_l && curr_s < curr_l {
                    if in_position && position_side == "long" {
                        // Exit long first
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason: "Short SMA crossed below Long SMA".to_string(),
                            timestamp_ms: current_ts,
                        });
                    }

                    if !in_position || position_side == "long" {
                        in_position = true;
                        position_side = "short";
                        let stop_loss = close_val + (atr_val * self.config.stop_loss_atr_mult);
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 1.0,
                            stop_loss: Some(stop_loss),
                            take_profit: None,
                            reason: "Short SMA crossed below Long SMA".to_string(),
                            timestamp_ms: current_ts,
                        });
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: SmaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        // Create 25 bars.
        // For SMA 9 and 21, we need 21 bars to warmup.
        // We'll simulate a downtrend then an uptrend to trigger a crossover.
        let mut close = Vec::new();
        let mut high = Vec::new();
        let mut low = Vec::new();
        let mut ts = Vec::new();

        let mut current_price = 100.0;

        // Downtrend for 15 bars
        for i in 0..15 {
            close.push(current_price);
            high.push(current_price + 1.0);
            low.push(current_price - 1.0);
            ts.push(1000 + i * 1000);
            current_price -= 1.0;
        }

        // Uptrend for 15 bars
        for i in 15..30 {
            close.push(current_price);
            high.push(current_price + 2.0);
            low.push(current_price - 1.0);
            ts.push(1000 + i * 1000);
            current_price += 2.0;
        }

        let mut df = df!(
            "close" => close,
            "high" => high,
            "low" => low,
            "timestamp_unix_ms" => ts,
        )
        .unwrap();
        df.with_column(
            df.column("timestamp_unix_ms").unwrap().cast(&DataType::Int64).unwrap()
        ).unwrap().clone()
    }

    #[tokio::test]
    async fn test_sma_crossover_signals() {
        let df = create_test_data();
        let config = SmaCrossoverConfig {
            short_period: 5,
            long_period: 10,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = SmaCrossover::new(config);

        let signals = strategy.generate_signals(&df).await.unwrap();

        // The exact number of signals depends on the mock data.
        // We just ensure it generates some signals and they are valid.
        assert!(!signals.is_empty(), "Should generate at least one signal");

        // We expect the first signal to be a buy (trend reverses up) or sell (trend reversed down)
        let first_entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(first_entry.is_some());

        if let Some(signal) = first_entry {
            assert!(
                signal.stop_loss.is_some(),
                "Entry signals should have a stop loss"
            );
        }
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let mut strategy = SmaCrossover::new(SmaCrossoverConfig::default());
        let params = serde_json::json!({
            "short_period": 10,
            "long_period": 50,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 20,
            "symbol": "ETHUSD"
        });

        strategy.update_params(params).await.unwrap();
        assert_eq!(strategy.config.short_period, 10);
        assert_eq!(strategy.config.long_period, 50);
        assert_eq!(strategy.config.stop_loss_atr_mult, 3.0);
        assert_eq!(strategy.config.atr_period, 20);
        assert_eq!(strategy.config.symbol, "ETHUSD");
    }

    #[tokio::test]
    async fn test_empty_data() {
        let df = DataFrame::default();
        let strategy = SmaCrossover::new(SmaCrossoverConfig::default());
        let result = strategy.generate_signals(&df).await;
        // The implementation should return an empty vec or an error.
        // Given `if data.height() < self.config.long_period { return Ok(vec![]); }`,
        // it should return Ok(vec![]).
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
