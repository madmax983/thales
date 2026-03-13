use crate::indicators::{atr, fisher_transform};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FisherTransformConfig {
    pub period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for FisherTransformConfig {}

pub struct FisherTransform {
    config: FisherTransformConfig,
}

impl FisherTransform {
    pub fn new(config: FisherTransformConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for FisherTransform {
    fn name(&self) -> &str {
        "FisherTransform"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let mut signals = Vec::new();

        if data.height() < self.config.period + 1 {
            return Ok(signals);
        }

        let (fisher, fisher_signal) = fisher_transform::calculate(data, self.config.period)?;
        let atr = atr::calculate(data, self.config.atr_period)?;

        let closes = data.column("close")?.f64()?;
        let times = data.column("timestamp_unix_ms")?.i64()?;

        let fisher_vals = fisher.f64()?;
        let fisher_sig_vals = fisher_signal.f64()?;
        let atr_vals = atr.f64()?;

        for i in self.config.period..data.height() {
            if let (Some(f_curr), Some(s_curr), Some(f_prev), Some(s_prev)) = (
                fisher_vals.get(i),
                fisher_sig_vals.get(i),
                fisher_vals.get(i - 1),
                fisher_sig_vals.get(i - 1),
            ) {
                let close = closes.get(i).unwrap_or(0.0);
                let timestamp_ms = times.get(i).unwrap_or(0);
                let current_atr = atr_vals.get(i).unwrap_or(0.0);

                // Long Entry: Fisher crosses ABOVE Signal (previous value), while BELOW oversold threshold
                // Note: Fisher Signal is the previous Fisher value.
                // So f_curr > s_curr is essentially Fisher(t) > Fisher(t-1) (turning up)
                // And f_prev <= s_prev is Fisher(t-1) <= Fisher(t-2) (was turning down or flat)
                if f_prev <= s_prev && f_curr > s_curr && f_curr < self.config.oversold_threshold {
                    let stop_loss = close - (current_atr * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(), // Can be dynamic
                        confidence: 0.8,              // Fixed confidence for now
                        stop_loss: Some(stop_loss),
                        take_profit: None,
                        reason: "Fisher Transform Oversold Crossover".to_string(),
                        timestamp_ms,
                    });
                }
                // Short Entry / Long Exit: Fisher crosses BELOW Signal, while ABOVE overbought threshold
                else if f_prev >= s_prev
                    && f_curr < s_curr
                    && f_curr > self.config.overbought_threshold
                {
                    let stop_loss = close + (current_atr * self.config.stop_loss_atr_mult);
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(stop_loss), // Stop loss for potential short entry
                        take_profit: None,
                        reason: "Fisher Transform Overbought Crossunder".to_string(),
                        timestamp_ms,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        if let Some(period) = params.get("period").and_then(|v| v.as_u64()) {
            self.config.period = period as usize;
        }
        if let Some(oversold) = params.get("oversold_threshold").and_then(|v| v.as_f64()) {
            self.config.oversold_threshold = oversold;
        }
        if let Some(overbought) = params.get("overbought_threshold").and_then(|v| v.as_f64()) {
            self.config.overbought_threshold = overbought;
        }
        if let Some(mult) = params.get("stop_loss_atr_mult").and_then(|v| v.as_f64()) {
            self.config.stop_loss_atr_mult = mult;
        }
        if let Some(atr_period) = params.get("atr_period").and_then(|v| v.as_u64()) {
            self.config.atr_period = atr_period as usize;
        }
        if let Some(symbol) = params.get("symbol").and_then(|v| v.as_str()) {
            self.config.symbol = symbol.to_string();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_default_config() -> FisherTransformConfig {
        FisherTransformConfig {
            period: 9,
            oversold_threshold: -1.5,
            overbought_threshold: 1.5,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        }
    }

    fn create_test_data() -> DataFrame {
        // Simple sequential data, Fisher will slowly climb
        // To trigger entry, we need Fisher to be < -1.5 and turn up.
        // Let's create an extreme drop followed by an up move
        let mut highs = vec![];
        let mut lows = vec![];
        let mut closes = vec![];
        let mut times = vec![];

        let mut price = 100.0;
        // 20 periods down
        for i in 0..20 {
            highs.push(Some(price + 1.0));
            lows.push(Some(price - 1.0));
            closes.push(Some(price));
            times.push(Some(i as i64 * 1000));
            price -= 2.0;
        }

        // 10 periods up (should trigger buy since it was dropping)
        for i in 20..30 {
            highs.push(Some(price + 1.0));
            lows.push(Some(price - 1.0));
            closes.push(Some(price));
            times.push(Some(i as i64 * 1000));
            price += 2.0;
        }

        // 10 periods down (should trigger sell since it was rising)
        for i in 30..40 {
            highs.push(Some(price + 1.0));
            lows.push(Some(price - 1.0));
            closes.push(Some(price));
            times.push(Some(i as i64 * 1000));
            price -= 2.0;
        }

        df!(
            "high" => &highs,
            "low" => &lows,
            "close" => &closes,
            "timestamp_unix_ms" => &times
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_fisher_transform_strategy() {
        let config = get_default_config();
        let strategy = FisherTransform::new(config);
        let df = create_test_data();

        let signals = strategy.generate_signals(&df).await.unwrap();
        // Since the data is synthetic, we just want to ensure it completes and generates valid Signal objects
        // Some entry/exit signals should be found at turning points
        assert!(!signals.is_empty(), "Should generate at least one signal");

        for signal in signals {
            assert_eq!(signal.symbol, "TEST");
            assert!(signal.stop_loss.is_some());
        }
    }

    #[tokio::test]
    async fn test_fisher_transform_empty_data() {
        let config = get_default_config();
        let strategy = FisherTransform::new(config);
        let df = DataFrame::empty();

        let result = strategy.generate_signals(&df).await;
        assert!(result.is_ok());
        let signals = result.unwrap();
        assert!(signals.is_empty());
    }

    #[tokio::test]
    async fn test_update_params() {
        let mut strategy = FisherTransform::new(get_default_config());
        let new_params = serde_json::json!({
            "period": 10,
            "oversold_threshold": -2.0,
            "overbought_threshold": 2.0,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 20,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await.unwrap();

        assert_eq!(strategy.config.period, 10);
        assert_eq!(strategy.config.oversold_threshold, -2.0);
        assert_eq!(strategy.config.overbought_threshold, 2.0);
        assert_eq!(strategy.config.stop_loss_atr_mult, 1.5);
        assert_eq!(strategy.config.atr_period, 20);
        assert_eq!(strategy.config.symbol, "BTCUSD");
    }
}
