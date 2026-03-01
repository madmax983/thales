use crate::indicators::{atr, vwma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VwapReversionConfig {
    pub vwma_period: usize,
    pub oversold_threshold_pct: f64,
    pub overbought_threshold_pct: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for VwapReversionConfig {}

pub struct VwapReversion {
    config: VwapReversionConfig,
}

impl VwapReversion {
    pub fn new(config: VwapReversionConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for VwapReversion {
    fn name(&self) -> &str {
        "VwapReversion"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
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

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);

        let oversold_thresh =
            Decimal::from_f64_retain(self.config.oversold_threshold_pct).unwrap_or(Decimal::ZERO);
        let overbought_thresh =
            Decimal::from_f64_retain(self.config.overbought_threshold_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let price_opt = close_arr.get(i);
            let prev_price_opt = close_arr.get(i - 1);
            let vwma_opt = vwma_arr.get(i);
            let prev_vwma_opt = vwma_arr.get(i - 1);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(price),
                Some(prev_price),
                Some(vwma_val),
                Some(prev_vwma_val),
                Some(atr_val),
            ) = (price_opt, prev_price_opt, vwma_opt, prev_vwma_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let prev_price_dec = Decimal::from_f64_retain(prev_price).unwrap_or(Decimal::ZERO);
                let vwma_dec = Decimal::from_f64_retain(vwma_val).unwrap_or(Decimal::ZERO);
                let prev_vwma_dec =
                    Decimal::from_f64_retain(prev_vwma_val).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                let lower_band = vwma_dec * (one_dec - oversold_thresh);
                let upper_band = vwma_dec * (one_dec + overbought_thresh);

                let prev_lower_band = prev_vwma_dec * (one_dec - oversold_thresh);
                let prev_upper_band = prev_vwma_dec * (one_dec + overbought_thresh);

                // Mean Reversion Entry (Long)
                // Buy when price drops below the lower band (oversold)
                if prev_price_dec >= prev_lower_band && price_dec < lower_band {
                    // Enter Long
                    let sl = price_dec - (atr_dec * sl_mult);
                    let tp = vwma_dec; // Target reverting to the mean

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "VWAP Reversion: Price {:.2} < Lower Band {:.2}",
                            price, lower_band
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Mean Reversion Entry (Short)
                // Sell when price goes above the upper band (overbought)
                else if prev_price_dec <= prev_upper_band && price_dec > upper_band {
                    // Enter Short
                    let sl = price_dec + (atr_dec * sl_mult);
                    let tp = vwma_dec; // Target reverting to the mean

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Entry Short
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "VWAP Reversion: Price {:.2} > Upper Band {:.2}",
                            price, upper_band
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Mean Reversion Exit
                // Since this is a simple signal generator, if price reverts to the mean, we exit.
                if prev_price_dec < prev_vwma_dec && price_dec >= vwma_dec {
                    // Price crossed above mean, exit Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "VWAP Reverted to Mean: Price {:.2} >= VWMA {:.2}",
                            price, vwma_val
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                if prev_price_dec > prev_vwma_dec && price_dec <= vwma_dec {
                    // Price crossed below mean, exit Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "VWAP Reverted to Mean: Price {:.2} <= VWMA {:.2}",
                            price, vwma_val
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: VwapReversionConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_vwap_reversion_signals() -> Result<()> {
        let config = VwapReversionConfig {
            vwma_period: 2,
            oversold_threshold_pct: 0.1,   // 10% below VWMA
            overbought_threshold_pct: 0.1, // 10% above VWMA
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = VwapReversion::new(config);

        // Let's manually trace VWMA (Period 2):
        // i=0: None
        // i=1: (10*100 + 10*100) / 200 = 10.0. Close 10.
        // i=2: (10*100 + 8*100) / 200 = 9.0. Close 8. Lower band = 9.0 * 0.9 = 8.1. Price is 8.0. 8.0 < 8.1. LONG ENTRY!
        //      Note: VWMA at i=2 is 9.0. VWMA at i=1 is 10.0.
        //      prev_price (i=1) = 10.0. prev_vwma (i=1) = 10.0. prev_lower_band = 9.0.
        //      prev_price >= prev_lower_band (10 >= 9.0)? YES.
        //      price < lower_band (8 < 8.1)? YES. LONG ENTRY.
        // i=3: (8*100 + 10*100) / 200 = 9.0. Close 10.
        //      prev_price (i=2) = 8.0. prev_vwma = 9.0.
        //      price (i=3) = 10.0. vwma = 9.0.
        //      prev_price < prev_vwma (8 < 9)? YES.
        //      price >= vwma (10 >= 9)? YES. LONG EXIT.
        // i=4: (10*100 + 13*100) / 200 = 11.5. Close 13. Upper band = 11.5 * 1.1 = 12.65.
        //      prev_price (i=3) = 10.0. prev_vwma = 9.0. prev_upper_band = 9.9.
        //      prev_price <= prev_upper_band (10 <= 9.9)? NO!
        //      Wait, 10 is NOT <= 9.9. So we miss the short entry?
        //      Ah, 10 <= 9.9 is false.

        // Let's fix the test data to ensure prev_price <= prev_upper_band for the short entry.
        // Let's make sure there are no other signals generated at unexpected times.
        // Wait, entries_long.len() = 2 ? Let's check i=0 and i=1.
        // i=1 (2000ms): price=10, vwma=10, lower=9. prev_price=10, prev_vwma=None.
        // Signal logic requires `prev_vwma_val` which is `vwma_arr.get(i - 1)`.
        // If i=1, prev is i=0. vwma at i=0 is None.
        // So no signal at i=1.

        // What about i=5?
        // i=5: (13*100 + 10*100)/200 = 11.5. Close 10.
        // lower band = 11.5 * 0.9 = 10.35.
        // price=10.0, lower_band=10.35. 10.0 < 10.35!
        // prev_price (i=4) = 13.0. prev_vwma (i=4) = 11.0. prev_lower_band = 11.0 * 0.9 = 9.9.
        // prev_price >= prev_lower_band (13.0 >= 9.9)? YES.
        // So i=5 generates a LONG ENTRY as well!
        // This is why entries_long.len() == 2.

        // Let's modify the data to avoid a long entry at i=5.
        // Change i=5 close to 11.0.
        // i=5: (13*100 + 11*100)/200 = 12.0. Close 11.
        // lower band = 12.0 * 0.9 = 10.8.
        // price=11.0 < 10.8? NO. So no long entry at i=5.
        // But we want a SHORT EXIT at i=5.
        // Short exit condition: prev_price > prev_vwma && price <= vwma.
        // prev_price (i=4) = 13.0. prev_vwma (i=4) = 11.0. (13 > 11)? YES.
        // price = 11.0. vwma = 12.0. (11 <= 12.0)? YES.
        // So i=5 will generate a SHORT EXIT, and NO LONG ENTRY.

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000],
            "open"  => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "high"  => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "low"   => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "close" => &[10.0, 10.0, 8.0, 9.0, 13.0, 11.0],
            "volume"=> &[100.0, 100.0, 100.0, 100.0, 100.0, 100.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Entry Long at i=2 (3000ms)
        let entries_long: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "buy")
            .collect();
        assert_eq!(
            entries_long.len(),
            1,
            "Expected 1 Long Entry. Got: {:?}",
            entries_long
        );
        assert_eq!(entries_long[0].timestamp_ms, 3000);

        // Exit Long at i=3 (4000ms)
        let exits_long: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit && s.side == "sell")
            .collect();
        // Since price 9.0 >= 8.5 (VWMA), exit is triggered at i=3
        assert_eq!(
            exits_long.len(),
            1,
            "Expected 1 Long Exit. Got: {:?}",
            exits_long
        );
        assert_eq!(exits_long[0].timestamp_ms, 4000);

        // Entry Short at i=4 (5000ms)
        let entries_short: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry && s.side == "sell")
            .collect();
        assert_eq!(
            entries_short.len(),
            1,
            "Expected 1 Short Entry. Got: {:?}",
            entries_short
        );
        assert_eq!(entries_short[0].timestamp_ms, 5000);

        // Exit Short at i=5 (6000ms)
        let exits_short: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit && s.side == "buy")
            .collect();
        assert_eq!(
            exits_short.len(),
            1,
            "Expected 1 Short Exit. Got: {:?}",
            exits_short
        );
        assert_eq!(exits_short[0].timestamp_ms, 6000);

        Ok(())
    }

    #[tokio::test]
    async fn test_vwap_reversion_update_params() -> Result<()> {
        let mut strategy = VwapReversion::new(VwapReversionConfig {
            vwma_period: 20,
            oversold_threshold_pct: 0.05,
            overbought_threshold_pct: 0.05,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "vwma_period": 50,
            "oversold_threshold_pct": 0.1,
            "overbought_threshold_pct": 0.1,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 20,
            "symbol": "NEW_TEST"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.vwma_period, 50);
        assert_eq!(strategy.config.symbol, "NEW_TEST");

        Ok(())
    }
}
