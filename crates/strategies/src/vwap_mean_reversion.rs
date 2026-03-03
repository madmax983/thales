use crate::indicators::vwap;
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VwapMeanReversionConfig {
    pub period: usize,
    pub entry_threshold_pct: f64,
    pub exit_threshold_pct: f64,
    pub stop_loss_pct: f64,
    pub symbol: String,
}

impl StrategyConfig for VwapMeanReversionConfig {}

pub struct VwapMeanReversion {
    config: VwapMeanReversionConfig,
}

impl VwapMeanReversion {
    pub fn new(config: VwapMeanReversionConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for VwapMeanReversion {
    fn name(&self) -> &str {
        "VwapMeanReversion"
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

        let vwap_series = vwap::calculate(data, self.config.period)?;
        let vwap_arr = vwap_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_pct_dec =
            Decimal::from_f64_retain(self.config.stop_loss_pct).unwrap_or(Decimal::ZERO);
        let entry_threshold_dec =
            Decimal::from_f64_retain(self.config.entry_threshold_pct).unwrap_or(Decimal::ZERO);
        let exit_threshold_dec =
            Decimal::from_f64_retain(self.config.exit_threshold_pct).unwrap_or(Decimal::ZERO);
        let one_dec = Decimal::ONE;

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let vwap_opt = vwap_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));

            if let (Some(price), Some(vwap_val)) = (price_opt, vwap_opt) {
                // Calculate thresholds
                let lower_band = vwap_val * (one_dec - entry_threshold_dec);
                let upper_band = vwap_val * (one_dec + exit_threshold_dec);

                // Entry condition: Price drops below VWAP lower threshold
                if price < lower_band {
                    let sl = price * (one_dec - stop_loss_pct_dec);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None, // TP could be the VWAP line itself
                        reason: format!(
                            "Price {:.2} < VWAP Lower Band {:.2}",
                            price, lower_band
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Exit condition: Price rises above VWAP upper threshold
                if price > upper_band {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Price {:.2} > VWAP Upper Band {:.2}",
                            price, upper_band
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: VwapMeanReversionConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_vwap_mean_reversion_signals() -> Result<()> {
        let config = VwapMeanReversionConfig {
            period: 3,
            entry_threshold_pct: 0.05, // 5% below VWAP
            exit_threshold_pct: 0.05,  // 5% above VWAP
            stop_loss_pct: 0.1,
            symbol: "TEST".to_string(),
        };
        let strategy = VwapMeanReversion::new(config);

        // Need data that generates VWAP and crosses bands
        // Let's manually trace VWAP.
        // Period = 3
        // i=0: P=10, V=100. VWAP=10
        // i=1: P=10, V=100. VWAP=10
        // i=2: P=10, V=100. VWAP=10
        // i=3: P=8,  V=100. VWAP=(10*100 + 10*100 + 8*100)/300 = 2800/300 = 9.33.
        //      Price is 8. Lower band = 9.33 * 0.95 = 8.86.
        //      8 < 8.86 -> ENTRY!
        // i=4: P=12, V=100. VWAP=(10*100 + 8*100 + 12*100)/300 = 3000/300 = 10.0.
        //      Price is 12. Upper band = 10.0 * 1.05 = 10.5.
        //      12 > 10.5 -> EXIT!

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "high"              => &[10.0, 10.0, 10.0, 8.0, 12.0],
            "low"               => &[10.0, 10.0, 10.0, 8.0, 12.0],
            "close"             => &[10.0, 10.0, 10.0, 8.0, 12.0],
            "volume"            => &[100.0, 100.0, 100.0, 100.0, 100.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // We expect Entry at i=3, Exit at i=4
        assert_eq!(signals.len(), 2);

        let entry = &signals[0];
        assert_eq!(entry.signal_type, SignalType::Entry);
        assert_eq!(entry.timestamp_ms, 4000); // i=3
        assert!(entry.reason.contains("VWAP Lower Band"));

        let exit = &signals[1];
        assert_eq!(exit.signal_type, SignalType::Exit);
        assert_eq!(exit.timestamp_ms, 5000); // i=4
        assert!(exit.reason.contains("VWAP Upper Band"));

        Ok(())
    }
}
