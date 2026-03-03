use crate::indicators::{atr, cmf};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaikinMoneyFlowConfig {
    pub period: usize,
    pub buy_threshold: f64,
    pub sell_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for ChaikinMoneyFlowConfig {}

pub struct ChaikinMoneyFlow {
    config: ChaikinMoneyFlowConfig,
}

impl ChaikinMoneyFlow {
    pub fn new(config: ChaikinMoneyFlowConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for ChaikinMoneyFlow {
    fn name(&self) -> &str {
        "ChaikinMoneyFlow"
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

        // Calculate CMF
        let cmf_series = cmf::calculate(data, self.config.period)?;
        let cmf_arr = cmf_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let stop_loss_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);

        // Iterate through data to identify crossovers
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);
            let cmf_val_opt = cmf_arr.get(i);
            let prev_cmf_val_opt = cmf_arr.get(i - 1);
            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (Some(price), Some(cmf_val), Some(prev_cmf), Some(atr_val)) =
                (price_opt, cmf_val_opt, prev_cmf_val_opt, atr_opt)
            {
                // Check Entry (CMF crosses above buy threshold)
                if prev_cmf <= self.config.buy_threshold && cmf_val > self.config.buy_threshold {
                    let sl_dist = atr_val * stop_loss_mult_dec;
                    let sl = price - sl_dist;

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None, // Momentum strategy, ride the trend until exit signal
                        reason: format!(
                            "CMF crossed above Buy Threshold: {:.3} > {:.3}",
                            cmf_val, self.config.buy_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Check Exit (CMF crosses below sell threshold)
                if prev_cmf >= self.config.sell_threshold && cmf_val < self.config.sell_threshold {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "CMF crossed below Sell Threshold: {:.3} < {:.3}",
                            cmf_val, self.config.sell_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: ChaikinMoneyFlowConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_chaikin_money_flow_signals() -> Result<()> {
        let config = ChaikinMoneyFlowConfig {
            period: 2,
            buy_threshold: 0.0,
            sell_threshold: 0.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = ChaikinMoneyFlow::new(config);

        // Need data that generates specific CMF values.
        // CMF(2):
        // MFM = ((C - L) - (H - C)) / (H - L)
        // i=0: H=12, L=10, C=10. MFM = ((0) - (2))/2 = -1.0. V=100. MFV=-100.
        // i=1: H=12, L=10, C=12. MFM = ((2) - (0))/2 = 1.0. V=100. MFV=100.
        //   -> CMF[1] = (-100 + 100)/200 = 0.0 (previous).
        // i=2: H=12, L=10, C=12. MFM = 1.0. V=100. MFV=100.
        //   -> CMF[2] = (100 + 100)/200 = 1.0. (Crosses above 0 -> Entry)
        // i=3: H=12, L=10, C=10. MFM = -1.0. V=100. MFV=-100.
        //   -> CMF[3] = (100 - 100)/200 = 0.0.
        // i=4: H=12, L=10, C=10. MFM = -1.0. V=100. MFV=-100.
        //   -> CMF[4] = (-100 - 100)/200 = -1.0. (Crosses below 0 -> Exit)

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "high" => &[12.0, 12.0, 12.0, 12.0, 12.0],
            "low" => &[10.0, 10.0, 10.0, 10.0, 10.0],
            "close" => &[10.0, 12.0, 12.0, 10.0, 10.0],
            "volume" => &[100.0, 100.0, 100.0, 100.0, 100.0]
        )?;

        let signals = strategy.generate_signals(&df).await?;

        let entries: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Entry)
            .collect();
        let exits: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == SignalType::Exit)
            .collect();

        // Expect Entry at i=2 (CMF goes from 0.0 to 1.0)
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp_ms, 3000);
        assert!(entries[0]
            .reason
            .contains("CMF crossed above Buy Threshold"));
        assert!(entries[0].stop_loss.is_some());

        // Expect Exit at i=4 (CMF goes from 0.0 to -1.0)
        assert_eq!(exits.len(), 1);
        assert_eq!(exits[0].timestamp_ms, 5000);
        assert!(exits[0].reason.contains("CMF crossed below Sell Threshold"));

        Ok(())
    }
}
