use crate::indicators::{atr, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmaCrossoverConfig {
    pub short_window: usize,
    pub long_window: usize,
    pub stop_loss_pct: f64, // Keep for fallback, but prefer ATR
    pub atr_period: usize,
    pub atr_mult: f64,
    pub symbol: String,
}

impl Default for SmaCrossoverConfig {
    fn default() -> Self {
        Self {
            short_window: 50,
            long_window: 200,
            stop_loss_pct: 0.05,
            atr_period: 14,
            atr_mult: 2.0,
            symbol: "BTC/USD".to_string(),
        }
    }
}

impl StrategyConfig for SmaCrossoverConfig {}

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
        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let short_sma_series = sma::calculate(data, self.config.short_window)?;
        let long_sma_series = sma::calculate(data, self.config.long_window)?;

        let short_sma = short_sma_series.f64()?;
        let long_sma = long_sma_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_pct_dec =
            Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.atr_mult).unwrap_or(Decimal::new(2, 0)); // Default 2.0 if missing

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);

            // Ensure we have SMA values
            let s_curr_opt = short_sma.get(i).and_then(Decimal::from_f64_retain);
            let l_curr_opt = long_sma.get(i).and_then(Decimal::from_f64_retain);
            let s_prev_opt = short_sma.get(i - 1).and_then(Decimal::from_f64_retain);
            let l_prev_opt = long_sma.get(i - 1).and_then(Decimal::from_f64_retain);

            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(sc), Some(lc), Some(sp), Some(lp), Some(price)) =
                (s_curr_opt, l_curr_opt, s_prev_opt, l_prev_opt, price_opt)
            {
                // Bearish Crossover (Exit) - Stateless
                if sc < lc && sp >= lp {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bearish Crossover: Short SMA {} < Long SMA {}",
                            sc.round_dp(2),
                            lc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Bullish Crossover (Entry) - Stateless
                if sc > lc && sp <= lp {
                    // Use ATR-based SL if available, else Fallback %
                    let sl = if let Some(atr_val) = atr_opt {
                        price - (atr_val * atr_mult_dec)
                    } else {
                        price * (one_dec - stop_loss_pct_dec)
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
                            "Bullish Crossover: Short SMA {} > Long SMA {}",
                            sc.round_dp(2),
                            lc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
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

    #[tokio::test]
    async fn test_sma_crossover_signals() -> Result<()> {
        // Setup: Short window 2, Long window 3.
        // Data designed to produce crossover.
        let config = SmaCrossoverConfig {
            short_window: 2,
            long_window: 3,
            stop_loss_pct: 0.2,
            atr_period: 2,
            atr_mult: 2.0,
            symbol: "TEST".to_string(),
        };
        let strategy = SmaCrossover::new(config);

        // Prices: 10, 10, 10, 12, 14, 10
        // Needs High/Low for ATR
        let closes = vec![10.0, 10.0, 10.0, 12.0, 14.0, 6.0];
        let highs = vec![10.5, 10.5, 10.5, 12.5, 14.5, 10.5];
        let lows = vec![9.5, 9.5, 9.5, 11.5, 13.5, 9.5];
        let timestamps = vec![1000i64, 2000, 3000, 4000, 5000, 6000];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect Entry at index 3, Exit at index 5.
        // Index 3 is bar 4. ATR should be available.
        assert!(signals.len() >= 2);

        let entry = &signals[0];
        assert_eq!(entry.signal_type, SignalType::Entry);
        assert_eq!(entry.timestamp_ms, 4000); // Index 3
        assert!(entry.stop_loss.is_some());

        let sl = entry.stop_loss.unwrap();
        assert!(sl > 0.0, "SL {} should be > 0.0", sl);

        Ok(())
    }

    #[tokio::test]
    async fn test_sma_crossover_empty_data() -> Result<()> {
        let config = SmaCrossoverConfig::default();
        let strategy = SmaCrossover::new(config);

        let df = DataFrame::default();
        let result = strategy.generate_signals(&df).await;

        assert!(result.is_err() || result.unwrap().is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_sma_crossover_update_params() -> Result<()> {
        let mut strategy = SmaCrossover::new(SmaCrossoverConfig::default());
        let new_params = serde_json::json!({
            "short_window": 10,
            "long_window": 20,
            "stop_loss_pct": 0.02,
            "atr_period": 14,
            "atr_mult": 1.5,
            "symbol": "ETH/USD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.short_window, 10);
        assert_eq!(strategy.config.long_window, 20);
        assert_eq!(strategy.config.stop_loss_pct, 0.02);
        assert_eq!(strategy.config.atr_period, 14);
        assert_eq!(strategy.config.atr_mult, 1.5);
        assert_eq!(strategy.config.symbol, "ETH/USD");

        Ok(())
    }
}
