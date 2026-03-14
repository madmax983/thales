use crate::indicators::{atr, sma, volume_oscillator};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

pub struct VolumeOscillatorTrend {
    config: VolumeOscillatorTrendConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VolumeOscillatorTrendConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub price_sma_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for VolumeOscillatorTrendConfig {}

impl VolumeOscillatorTrend {
    pub fn new(config: VolumeOscillatorTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for VolumeOscillatorTrend {
    fn name(&self) -> &str {
        "VolumeOscillatorTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        let close_series = data.column("close")?;
        let close = close_series.f64()?;
        let timestamp = data.column("timestamp_unix_ms")?.i64()?;

        if close.len() <= self.config.long_period.max(self.config.price_sma_period).max(self.config.atr_period) {
            return Ok(vec![]);
        }

        let vo_series = volume_oscillator::calculate(data, self.config.short_period, self.config.long_period)?;
        let vo = vo_series.f64()?;

        let sma_series = sma::calculate(data, self.config.price_sma_period)?;
        let sma = sma_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr = atr_series.f64()?;

        let mut signals = Vec::new();

        for i in 1..close.len() {
            let prev_vo = vo.get(i - 1);
            let curr_vo = vo.get(i);
            let curr_close = close.get(i);
            let curr_sma = sma.get(i);
            let curr_atr = atr.get(i);
            let ts = timestamp.get(i);

            if let (Some(p_vo), Some(c_vo), Some(price), Some(sma_val), Some(atr_val), Some(time_ms)) =
                (prev_vo, curr_vo, curr_close, curr_sma, curr_atr, ts)
            {
                // Buy: VO crosses above 0 and Price is above SMA
                if p_vo <= 0.0 && c_vo > 0.0 && price > sma_val {
                    let stop_loss = price - (atr_val * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: "VO crossed above 0 and price above SMA".to_string(),
                        timestamp_ms: time_ms,
                    });
                }
                // Sell: VO crosses above 0 and Price is below SMA (Short)
                else if p_vo <= 0.0 && c_vo > 0.0 && price < sma_val {
                    let stop_loss = price + (atr_val * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: "VO crossed above 0 and price below SMA".to_string(),
                        timestamp_ms: time_ms,
                    });
                }

                // Exit Long: VO crosses below 0
                // For simplicity, emit a sell if it was a long setup (price > sma)
                // However, without state, an exit signal should probably just close.
                // Since this framework emits generic exits, we can just emit both or handle it appropriately.
                // Let's emit a generic exit signal (if we don't know the position side).
                // Or better, since it's an exit, we can just say `SignalType::Exit` with `side: "sell"` and `side: "buy"` so it closes whichever is open.
                // But typically, a momentum loss (VO < 0) just exits the position.
                // Let's emit both exit sides when VO drops < 0, as we don't hold state here.
                if p_vo >= 0.0 && c_vo < 0.0 {
                     signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "VO crossed below 0 (Exit Long)".to_string(),
                        timestamp_ms: time_ms,
                    });
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "VO crossed below 0 (Exit Short)".to_string(),
                        timestamp_ms: time_ms,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: VolumeOscillatorTrendConfig = serde_json::from_value(params)?;
        // Validation
        if new_config.short_period == 0 || new_config.long_period == 0 || new_config.price_sma_period == 0 || new_config.atr_period == 0 {
            anyhow::bail!("Periods must be > 0");
        }
        if new_config.short_period >= new_config.long_period {
            anyhow::bail!("Short period must be < long period");
        }

        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_buy_test_data() -> DataFrame {
        df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "high" => &[10.0, 11.0, 12.0, 15.0, 14.0],
            "low" => &[9.0, 10.0, 11.0, 11.0, 13.0],
            "close" => &[10.0, 11.0, 12.0, 14.0, 15.0],
            // For long_period=3, short=2
            // VO = short - long
            // i=0: v=10
            // i=1: v=10
            // i=2: v=10 (long=10, short=10, vo=0)
            // i=3: v=50 (long=(10+10+50)/3=23.3, short=(10+50)/2=30, vo > 0)
            // SMA(2) on close: i=3: (12+14)/2 = 13. price=14 > 13.
            "volume" => &[10.0, 10.0, 10.0, 50.0, 10.0]
        ).unwrap()
    }

    fn create_sell_test_data() -> DataFrame {
        df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "high" => &[15.0, 14.0, 13.0, 12.0, 11.0],
            "low" => &[14.0, 13.0, 12.0, 11.0, 10.0],
            "close" => &[15.0, 14.0, 13.0, 11.0, 10.0],
            // VO crosses 0.
            // SMA(2) on close: i=3: (13+11)/2 = 12. price=11 < 12.
            "volume" => &[10.0, 10.0, 10.0, 50.0, 10.0]
        ).unwrap()
    }

    fn create_exit_test_data() -> DataFrame {
        df!(
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000],
            "high" => &[15.0, 14.0, 13.0, 12.0, 11.0],
            "low" => &[14.0, 13.0, 12.0, 11.0, 10.0],
            "close" => &[15.0, 14.0, 13.0, 11.0, 10.0],
            // At i=2: volume drops, VO < 0
            // i=0: 50
            // i=1: 50
            // i=2: 50 (long=50, short=50, vo=0)
            // i=3: 10 (long=(50+50+10)/3=36.6, short=(50+10)/2=30, vo < 0)
            "volume" => &[50.0, 50.0, 50.0, 10.0, 50.0]
        ).unwrap()
    }

    #[tokio::test]
    async fn test_vo_trend_buy_signal() -> Result<()> {
        let data = create_buy_test_data();

        let config = VolumeOscillatorTrendConfig {
            short_period: 2,
            long_period: 3,
            price_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };

        let strategy = VolumeOscillatorTrend::new(config);
        let signals = strategy.generate_signals(&data).await?;

        let buy_signal = signals.iter().find(|s| s.side == "buy" && s.signal_type == SignalType::Entry);
        assert!(buy_signal.is_some(), "Expected a buy entry signal when VO crosses 0 and Price > SMA");

        Ok(())
    }

    #[tokio::test]
    async fn test_vo_trend_sell_signal() -> Result<()> {
        let data = create_sell_test_data();

        let config = VolumeOscillatorTrendConfig {
            short_period: 2,
            long_period: 3,
            price_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };

        let strategy = VolumeOscillatorTrend::new(config);
        let signals = strategy.generate_signals(&data).await?;

        let sell_signal = signals.iter().find(|s| s.side == "sell" && s.signal_type == SignalType::Entry);
        assert!(sell_signal.is_some(), "Expected a sell entry signal when VO crosses 0 and Price < SMA");

        Ok(())
    }

    #[tokio::test]
    async fn test_vo_trend_exit_signal() -> Result<()> {
        let data = create_exit_test_data();

        let config = VolumeOscillatorTrendConfig {
            short_period: 2,
            long_period: 3,
            price_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };

        let strategy = VolumeOscillatorTrend::new(config);
        let signals = strategy.generate_signals(&data).await?;

        let exit_signal = signals.iter().find(|s| s.signal_type == SignalType::Exit);
        assert!(exit_signal.is_some(), "Expected an exit signal when VO crosses below 0");

        Ok(())
    }

    #[tokio::test]
    async fn test_vo_empty_data() -> Result<()> {
        let data = DataFrame::default();
        let config = VolumeOscillatorTrendConfig {
            short_period: 2,
            long_period: 3,
            price_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = VolumeOscillatorTrend::new(config);
        let signals = strategy.generate_signals(&data).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_vo_parameter_validation() -> Result<()> {
        let mut strategy = VolumeOscillatorTrend::new(VolumeOscillatorTrendConfig {
            short_period: 2,
            long_period: 3,
            price_sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        });

        let valid_params = serde_json::json!({
            "short_period": 3,
            "long_period": 5,
            "price_sma_period": 10,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 5,
            "symbol": "BTCUSD"
        });

        assert!(strategy.update_params(valid_params).await.is_ok());

        let invalid_params = serde_json::json!({
            "short_period": 5,
            "long_period": 3, // short >= long
            "price_sma_period": 10,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 5,
            "symbol": "BTCUSD"
        });

        assert!(strategy.update_params(invalid_params).await.is_err());

        Ok(())
    }
}