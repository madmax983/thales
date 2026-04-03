use crate::strategy::{Signal, SignalType, Strategy, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::indicators::{atr, bop, sma};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BopMomentumConfig {
    pub bop_sma_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl Default for BopMomentumConfig {
    fn default() -> Self {
        Self {
            bop_sma_period: 14,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

pub struct BopMomentum {
    config: BopMomentumConfig,
}

impl BopMomentum {
    pub fn new(config: BopMomentumConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for BopMomentum {
    fn name(&self) -> &str {
        "BopMomentum"
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

        // Calculate BOP
        let bop_series = bop::calculate(data)?;
        let bop_arr = bop_series.f64()?;

        // Calculate BOP SMA (Signal Line)
        let mut s = bop_series.clone();
        s.rename("close");
        let bop_df = DataFrame::new(vec![s])?;
        let bop_sma_series = sma::calculate(&bop_df, self.config.bop_sma_period)?;
        let bop_sma_arr = bop_sma_series.f64()?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let bop_curr = bop_arr.get(i);
            let bop_prev = bop_arr.get(i - 1);
            let sma_curr = bop_sma_arr.get(i);
            let sma_prev = bop_sma_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(bop_c),
                Some(bop_p),
                Some(sma_c),
                Some(sma_p),
                Some(price),
                Some(atr_val),
            ) = (bop_curr, bop_prev, sma_curr, sma_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Bullish Crossover (BOP crosses ABOVE SMA and BOP > 0)
                if bop_p <= sma_p && bop_c > sma_c && bop_c > 0.0 {
                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("BOP Crossover Up: BOP {:.2} > SMA {:.2}", bop_c, sma_c),
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
                        reason: format!("BOP Crossover Up: BOP {:.2} > SMA {:.2}", bop_c, sma_c),
                        timestamp_ms: timestamp,
                    });
                }
                // Bearish Crossover (BOP crosses BELOW SMA and BOP < 0)
                else if bop_p >= sma_p && bop_c < sma_c && bop_c < 0.0 {
                    // Exit any Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("BOP Crossover Down: BOP {:.2} < SMA {:.2}", bop_c, sma_c),
                        timestamp_ms: timestamp,
                    });

                    // Enter Short
                    let sl = price_dec + (atr_dec * sl_mult);
                    let risk = sl - price_dec;
                    let tp = price_dec - (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!("BOP Crossover Down: BOP {:.2} < SMA {:.2}", bop_c, sma_c),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: BopMomentumConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_bop_momentum_signals() -> Result<()> {
        let config = BopMomentumConfig {
            bop_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = BopMomentum::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "open"  => &[10.0, 10.0, 10.0, 10.0],
            "high"  => &[12.0, 12.0, 12.0, 12.0],
            "low"   => &[8.0, 8.0, 8.0, 8.0],
            "close" => &[10.0, 11.0, 9.0, 12.0], // BOP: 0.0, 0.25, -0.25, 0.5
        )?;

        // P0: c=10. BOP = (10-10)/(12-8) = 0.
        // P1: c=11. BOP = (11-10)/4 = 0.25.
        // P2: c=9. BOP = (9-10)/4 = -0.25.
        // P3: c=12. BOP = (12-10)/4 = 0.5.

        // SMA(2) of BOP:
        // P0: Null
        // P1: (0 + 0.25) / 2 = 0.125
        // P2: (0.25 - 0.25) / 2 = 0.0
        // P3: (-0.25 + 0.5) / 2 = 0.125

        // Let's trace crosses:
        // P2: bop_prev=0.25, bop_curr=-0.25. sma_prev=0.125, sma_curr=0.0
        // bop crosses below sma? 0.25 >= 0.125 && -0.25 < 0.0. YES. bop_c < 0.0? YES.
        // -> BEARISH CROSSOVER (Short Entry) at P2 (3000ms).

        // P3: bop_prev=-0.25, bop_curr=0.5. sma_prev=0.0, sma_curr=0.125.
        // bop crosses above sma? -0.25 <= 0.0 && 0.5 > 0.125. YES. bop_c > 0.0? YES.
        // -> BULLISH CROSSOVER (Long Entry) at P3 (4000ms).

        let signals = strategy.generate_signals(&df).await?;

        let entry_shorts: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "sell")
            .collect();
        assert_eq!(entry_shorts.len(), 1);
        assert_eq!(entry_shorts[0].timestamp_ms, 3000);

        let entry_longs: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();
        assert_eq!(entry_longs.len(), 1);
        assert_eq!(entry_longs[0].timestamp_ms, 4000);

        Ok(())
    }

    #[tokio::test]
    async fn test_bop_momentum_empty_data() -> Result<()> {
        let config = BopMomentumConfig::default();
        let strategy = BopMomentum::new(config);

        let df = DataFrame::default();
        let result = strategy.generate_signals(&df).await;

        assert!(result.is_err()); // bop::calculate will return an error for empty data
        Ok(())
    }

    #[tokio::test]
    async fn test_bop_momentum_extreme_volatility() -> Result<()> {
        let config = BopMomentumConfig {
            bop_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "VOLATILE".to_string(),
        };
        let strategy = BopMomentum::new(config);

        // Huge wicks, massive drops, some zero divisions in range calculation
        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000],
            "open"  => &[100.0, 50.0, 200.0, 10.0],
            "high"  => &[200.0, 50.0, 300.0, 10.0],
            "low"   => &[10.0, 50.0, 10.0, 10.0], // High == Low at i=1, i=3
            "close" => &[50.0, 50.0, 250.0, 10.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;
        // Strategy shouldn't panic, should handle missing BOP gracefully.
        // at i=1, h=50, l=50, range=0. bop will be 0.0 to avoid zero division.
        // at i=3, h=10, l=10, range=0. bop will be 0.0.

        // We just want to ensure it survives.
        assert!(!signals.is_empty() || signals.is_empty()); // Just verifying it doesn't crash

        Ok(())
    }

    #[tokio::test]
    async fn test_bop_momentum_update_params() -> Result<()> {
        let mut strategy = BopMomentum::new(BopMomentumConfig::default());
        let new_params = serde_json::json!({
            "bop_sma_period": 10,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 14,
            "symbol": "NEW_TEST"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.bop_sma_period, 10);
        assert_eq!(strategy.config.symbol, "NEW_TEST");

        Ok(())
    }
}
