use crate::indicators::{atr, sma, vwma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VwmaCrossoverConfig {
    pub vwma_period: usize,
    pub sma_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for VwmaCrossoverConfig {}

pub struct VwmaCrossover {
    config: VwmaCrossoverConfig,
}

impl VwmaCrossover {
    pub fn new(config: VwmaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for VwmaCrossover {
    fn name(&self) -> &str {
        "VwmaCrossover"
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

        // Calculate VWMA
        let vwma_series = vwma::calculate(data, self.config.vwma_period)?;
        let vwma_arr = vwma_series.f64()?;

        // Calculate SMA
        let sma_series = sma::calculate(data, self.config.sma_period)?;
        let sma_arr = sma_series.f64()?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let vwma_curr = vwma_arr.get(i);
            let vwma_prev = vwma_arr.get(i - 1);
            let sma_curr = sma_arr.get(i);
            let sma_prev = sma_arr.get(i - 1);

            let price_opt = close_arr.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(vwma_c),
                Some(vwma_p),
                Some(sma_c),
                Some(sma_p),
                Some(price),
                Some(atr_val),
            ) = (vwma_curr, vwma_prev, sma_curr, sma_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Bullish Crossover (VWMA crosses ABOVE SMA)
                if vwma_p <= sma_p && vwma_c > sma_c {
                    // Exit any Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short (Buy to cover)
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!("VWMA Crossover Up: VWMA {:.2} > SMA {:.2}", vwma_c, sma_c),
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
                        reason: format!("VWMA Crossover Up: VWMA {:.2} > SMA {:.2}", vwma_c, sma_c),
                        timestamp_ms: timestamp,
                    });
                }
                // Bearish Crossover (VWMA crosses BELOW SMA)
                else if vwma_p >= sma_p && vwma_c < sma_c {
                    // Exit any Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "VWMA Crossover Down: VWMA {:.2} < SMA {:.2}",
                            vwma_c, sma_c
                        ),
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
                        reason: format!(
                            "VWMA Crossover Down: VWMA {:.2} < SMA {:.2}",
                            vwma_c, sma_c
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: VwmaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_vwma_crossover_signals() -> Result<()> {
        let config = VwmaCrossoverConfig {
            vwma_period: 2,
            sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = VwmaCrossover::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000],
            "open"  => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "high"  => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "low"   => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "close" => &[10.0, 10.0, 20.0, 20.0, 10.0, 10.0],
            "volume"=> &[100.0, 100.0, 200.0, 100.0, 200.0, 100.0]
        )?;

        // Close: 10, 10, 20, 20, 10, 10
        // SMA(2): -, 10, 15, 20, 15, 10
        // Volume: 100, 100, 200, 100, 200, 100
        // VWMA(2):
        // i=1 (1000+1000)/200 = 10
        // i=2 (1000+4000)/300 = 16.66  (16.66 > SMA 15) -> LONG ENTRY
        // i=3 (4000+2000)/300 = 20     (20 = SMA 20)
        // i=4 (2000+1000)/300 = 10     (10 < SMA 15) -> SHORT ENTRY

        let signals = strategy.generate_signals(&df).await?;

        // Entry Long at i=2 (3000ms): VWMA crosses above SMA
        let entries_long: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();
        assert_eq!(entries_long.len(), 1);
        assert_eq!(entries_long[0].timestamp_ms, 3000);

        // Entry Short at i=4 (5000ms): VWMA crosses below SMA
        let entries_short: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "sell")
            .collect();
        assert_eq!(entries_short.len(), 1);
        assert_eq!(entries_short[0].timestamp_ms, 5000);

        Ok(())
    }

    #[tokio::test]
    async fn test_vwma_update_params() -> Result<()> {
        let mut strategy = VwmaCrossover::new(VwmaCrossoverConfig {
            vwma_period: 20,
            sma_period: 20,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "vwma_period": 50,
            "sma_period": 50,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 20,
            "symbol": "NEW_TEST"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.vwma_period, 50);
        assert_eq!(strategy.config.sma_period, 50);
        assert_eq!(strategy.config.symbol, "NEW_TEST");

        Ok(())
    }
}
