use crate::indicators::{atr, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripleSmaCrossoverConfig {
    pub short_period: usize,
    pub medium_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for TripleSmaCrossoverConfig {}

impl Default for TripleSmaCrossoverConfig {
    fn default() -> Self {
        Self {
            short_period: 9,
            medium_period: 21,
            long_period: 50,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "UNKNOWN".to_string(),
        }
    }
}

pub struct TripleSmaCrossover {
    config: TripleSmaCrossoverConfig,
}

impl TripleSmaCrossover {
    pub fn new(config: TripleSmaCrossoverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for TripleSmaCrossover {
    fn name(&self) -> &str {
        "TripleSmaCrossover"
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

        let short_sma_series = sma::calculate(data, self.config.short_period)?;
        let medium_sma_series = sma::calculate(data, self.config.medium_period)?;
        let long_sma_series = sma::calculate(data, self.config.long_period)?;

        let short_sma = short_sma_series.f64()?;
        let medium_sma = medium_sma_series.f64()?;
        let long_sma = long_sma_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let atr_mult_dec =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::new(2, 0));

        // Iterate through data
        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(Decimal::from_f64_retain);

            // Ensure we have SMA values
            let s_curr_opt = short_sma.get(i).and_then(Decimal::from_f64_retain);
            let m_curr_opt = medium_sma.get(i).and_then(Decimal::from_f64_retain);
            let l_curr_opt = long_sma.get(i).and_then(Decimal::from_f64_retain);

            let s_prev_opt = short_sma.get(i - 1).and_then(Decimal::from_f64_retain);
            let m_prev_opt = medium_sma.get(i - 1).and_then(Decimal::from_f64_retain);
            let l_prev_opt = long_sma.get(i - 1).and_then(Decimal::from_f64_retain);

            let atr_opt = atr_arr.get(i).and_then(Decimal::from_f64_retain);

            if let (
                Some(sc),
                Some(mc),
                Some(lc),
                Some(sp),
                Some(mp),
                Some(lp),
                Some(price),
                Some(atr_val),
            ) = (
                s_curr_opt, m_curr_opt, l_curr_opt, s_prev_opt, m_prev_opt, l_prev_opt, price_opt,
                atr_opt,
            ) {
                // Bullish Entry (Long) - Stateless
                // Condition: Fast > Medium > Slow, and previously not all true
                let curr_bullish = sc > mc && mc > lc;
                let prev_bullish = sp > mp && mp > lp;

                if curr_bullish && !prev_bullish {
                    let sl = price - (atr_val * atr_mult_dec);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "Bullish Triple Crossover: Short {} > Medium {} > Long {}",
                            sc.round_dp(2),
                            mc.round_dp(2),
                            lc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Bearish Entry (Short) - Stateless
                let curr_bearish = sc < mc && mc < lc;
                let prev_bearish = sp < mp && mp < lp;

                if curr_bearish && !prev_bearish {
                    let sl = price + (atr_val * atr_mult_dec);
                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: None,
                        reason: format!(
                            "Bearish Triple Crossover: Short {} < Medium {} < Long {}",
                            sc.round_dp(2),
                            mc.round_dp(2),
                            lc.round_dp(2)
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Exit Long
                let curr_exit_long = sc < mc;
                let prev_exit_long = sp >= mp;

                if curr_exit_long && prev_exit_long {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Exit Long: Short {} < Medium {}",
                            sc.round_dp(2),
                            mc.round_dp(2),
                        ),
                        timestamp_ms: timestamp,
                    });
                }

                // Exit Short
                let curr_exit_short = sc > mc;
                let prev_exit_short = sp <= mp;

                if curr_exit_short && prev_exit_short {
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Exit Short: Short {} > Medium {}",
                            sc.round_dp(2),
                            mc.round_dp(2),
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: TripleSmaCrossoverConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_triple_sma_crossover_signals() -> Result<()> {
        let config = TripleSmaCrossoverConfig {
            short_period: 2,
            medium_period: 3,
            long_period: 4,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = TripleSmaCrossover::new(config);

        // Prices for bullish crossover: Fast > Medium > Slow
        let closes = vec![10.0, 10.0, 10.0, 10.0, 12.0, 14.0, 16.0, 18.0, 10.0, 8.0];
        let highs = vec![10.5, 10.5, 10.5, 10.5, 12.5, 14.5, 16.5, 18.5, 10.5, 8.5];
        let lows = vec![9.5, 9.5, 9.5, 9.5, 11.5, 13.5, 15.5, 17.5, 9.5, 7.5];
        let timestamps = vec![
            1000i64, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000,
        ];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "close" => closes,
            "high" => highs,
            "low" => lows
        )?;

        let signals = strategy.generate_signals(&df).await?;

        assert!(signals.len() >= 2);

        let entry = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "buy");
        assert!(entry.is_some());

        let exit = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Exit && s.side == "sell");
        assert!(exit.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let mut strategy = TripleSmaCrossover::new(TripleSmaCrossoverConfig::default());
        let new_params = serde_json::json!({
            "short_period": 5,
            "medium_period": 10,
            "long_period": 20,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await.unwrap();
        assert_eq!(strategy.config.short_period, 5);
        assert_eq!(strategy.config.symbol, "BTCUSD");
    }
}
