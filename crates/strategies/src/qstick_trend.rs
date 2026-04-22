use crate::indicators::qstick;
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QstickTrendConfig {
    pub period: usize,
    pub symbol: String,
    pub stop_loss_pct: f64,
    pub max_position_size: f64,
}

impl StrategyConfig for QstickTrendConfig {}

pub struct QstickTrend {
    config: QstickTrendConfig,
}

impl QstickTrend {
    pub fn new(config: QstickTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for QstickTrend {
    fn name(&self) -> &str {
        "QstickTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?.clone();
        let close_arr = close_series.f64()?;

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        let qstick_series = qstick::calculate(data, self.config.period)?;
        let qstick_arr = qstick_series.f64()?;

        let mut signals = Vec::new();

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i);

            let qs_curr_opt = qstick_arr.get(i);
            let qs_prev_opt = qstick_arr.get(i - 1);

            if let (Some(price), Some(qs_curr), Some(qs_prev)) = (price_opt, qs_curr_opt, qs_prev_opt) {
                // Bullish Crossover (Qstick crosses above 0)
                if qs_prev <= 0.0 && qs_curr > 0.0 {
                    let sl = price * (1.0 - self.config.stop_loss_pct);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: self.config.max_position_size.to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: None, // Can add TP logic if needed
                        reason: format!("Qstick Crossover Up: {:.2}", qs_curr),
                        timestamp_ms: timestamp,
                    });
                }
                // Bearish Crossover (Qstick crosses below 0)
                else if qs_prev >= 0.0 && qs_curr < 0.0 {
                    let sl = price * (1.0 + self.config.stop_loss_pct);
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: None,
                        reason: format!("Qstick Crossover Down: {:.2}", qs_curr),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: QstickTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_qstick_signals() -> Result<()> {
        let config = QstickTrendConfig {
            period: 2,
            symbol: "TEST".to_string(),
            stop_loss_pct: 0.05,
            max_position_size: 1.0,
        };
        let strategy = QstickTrend::new(config);

        let df = df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "open" => &[10.0, 11.0, 10.0, 10.0, 11.0],
            "close" => &[11.0, 10.0, 12.0, 13.0, 9.0]
        )?;

        // diff: [1, -1, 2, 3, -2]
        // period=2
        // sum(0,1)=0 -> qs[1]=0
        // sum(1,2)=1 -> qs[2]=0.5 (crosses above 0 -> BUY)
        // sum(2,3)=5 -> qs[3]=2.5
        // sum(3,4)=1 -> qs[4]=0.5

        let signals = strategy.generate_signals(&df).await?;
        assert!(!signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = QstickTrend::new(QstickTrendConfig {
            period: 2,
            symbol: "TEST".to_string(),
            stop_loss_pct: 0.05,
            max_position_size: 100.0,
        });

        let new_params = serde_json::json!({
            "period": 5,
            "symbol": "BTCUSD",
            "stop_loss_pct": 0.1,
            "max_position_size": 50.0
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.period, 5);
        assert_eq!(strategy.config.symbol, "BTCUSD");
        assert_eq!(strategy.config.stop_loss_pct, 0.1);
        Ok(())
    }
}
