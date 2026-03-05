use anyhow::{Context, Result};
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::strategy::{Signal, SignalType, Strategy, StrategyType};

/// Configuration for the StochRSI Mean Reversion Strategy
#[derive(Debug, Clone)]
pub struct StochRsiMeanReversionConfig {
    /// Lookback period for RSI
    pub rsi_period: usize,
    /// Lookback period for StochRSI
    pub stoch_period: usize,
    /// Smoothing period for %K
    pub k_period: usize,
    /// Smoothing period for %D
    pub d_period: usize,
    /// Threshold for oversold condition (e.g., 20)
    pub oversold_threshold: f64,
    /// Threshold for overbought condition (e.g., 80)
    pub overbought_threshold: f64,
    /// Stop loss ATR multiplier
    pub stop_loss_atr_mult: Decimal,
    /// ATR period for stop loss calculation
    pub atr_period: usize,
    /// Symbol to trade
    pub symbol: String,
}

impl Default for StochRsiMeanReversionConfig {
    fn default() -> Self {
        Self {
            rsi_period: 14,
            stoch_period: 14,
            k_period: 3,
            d_period: 3,
            oversold_threshold: 20.0,
            overbought_threshold: 80.0,
            stop_loss_atr_mult: Decimal::from(2),
            atr_period: 14,
            symbol: "XXBTZUSD".to_string(), // Default Kraken BTC/USD symbol
        }
    }
}

/// StochRSI Mean Reversion Strategy
///
/// Applies the Stochastic oscillator formula to the Relative Strength Index (RSI)
/// to identify overbought and oversold conditions with greater sensitivity.
pub struct StochRsiMeanReversion {
    config: StochRsiMeanReversionConfig,
}

impl StochRsiMeanReversion {
    /// Create a new StochRsiMeanReversion strategy with the given configuration
    pub fn new(config: StochRsiMeanReversionConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for StochRsiMeanReversion {
    fn name(&self) -> &str {
        "StochRsiMeanReversion"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    async fn update_params(&mut self, _params: serde_json::Value) -> Result<()> {
        Ok(())
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            anyhow::bail!("Data cannot be empty");
        }

        let mut signals = Vec::new();

        // Ensure we have 'close' and 'timestamp_unix_ms' columns
        let close = data
            .column("close")
            .context("DataFrame must contain 'close' column")?
            .f64()?;
        let timestamps = data
            .column("timestamp_unix_ms")
            .context("DataFrame must contain 'timestamp_unix_ms' column")?
            .i64()?;

        // Calculate StochRSI
        let (k_series, d_series) = crate::indicators::stoch_rsi::calculate(
            data,
            self.config.rsi_period,
            self.config.stoch_period,
            self.config.k_period,
            self.config.d_period,
        )?;
        let k_arr = k_series.f64()?;
        let d_arr = d_series.f64()?;

        // Calculate ATR for stop loss
        let atr_series = crate::indicators::atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut position: Option<String> = None; // "buy" or "sell"
        let mut stop_loss: Option<Decimal> = None;

        for i in 1..data.height() {
            let k_c_opt = k_arr.get(i);
            let d_c_opt = d_arr.get(i);
            let k_p_opt = k_arr.get(i - 1);
            let d_p_opt = d_arr.get(i - 1);

            let close_opt = close.get(i);
            let atr_opt = atr_arr.get(i);
            let timestamp_opt = timestamps.get(i);

            if let (
                Some(k_c),
                Some(d_c),
                Some(k_p),
                Some(d_p),
                Some(current_close),
                Some(atr),
                Some(ts),
            ) = (
                k_c_opt,
                d_c_opt,
                k_p_opt,
                d_p_opt,
                close_opt,
                atr_opt,
                timestamp_opt,
            ) {
                let current_close_dec = Decimal::from_f64(current_close).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64(atr).unwrap_or(Decimal::ZERO);

                // Entry logic
                if position.is_none() {
                    // Long Entry: %K crosses above %D in the oversold region
                    if k_p <= d_p && k_c > d_c && k_c < self.config.oversold_threshold {
                        position = Some("buy".to_string());
                        let sl = current_close_dec - (atr_dec * self.config.stop_loss_atr_mult);
                        stop_loss = Some(sl);

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100.0".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: None,
                            reason: format!(
                                "StochRSI %K ({:.2}) crossed above %D ({:.2}) below oversold threshold",
                                k_c, d_c
                            ),
                            timestamp_ms: ts,
                        });
                        continue;
                    }

                    // Short Entry: %K crosses below %D in the overbought region
                    if k_p >= d_p && k_c < d_c && k_c > self.config.overbought_threshold {
                        position = Some("sell".to_string());
                        let sl = current_close_dec + (atr_dec * self.config.stop_loss_atr_mult);
                        stop_loss = Some(sl);

                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "100.0".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: None,
                            reason: format!(
                                "StochRSI %K ({:.2}) crossed below %D ({:.2}) above overbought threshold",
                                k_c, d_c
                            ),
                            timestamp_ms: ts,
                        });
                        continue;
                    }
                }

                // Exit logic
                if let Some(ref pos) = position {
                    let mut should_exit = false;
                    let mut exit_reason = String::new();

                    if pos == "buy" {
                        // Exit on Stop Loss
                        if let Some(sl) = stop_loss {
                            if current_close_dec <= sl {
                                should_exit = true;
                                exit_reason = "Stop Loss hit".to_string();
                            }
                        }
                        // Exit condition: %K crosses below %D in overbought region
                        if !should_exit
                            && k_p >= d_p
                            && k_c < d_c
                            && k_c > self.config.overbought_threshold
                        {
                            should_exit = true;
                            exit_reason = format!(
                                "StochRSI %K ({:.2}) crossed below %D ({:.2}) above overbought threshold",
                                k_c, d_c
                            );
                        }

                        if should_exit {
                            signals.push(Signal {
                                signal_type: SignalType::Exit,
                                symbol: self.config.symbol.clone(),
                                side: "sell".to_string(),
                                size_hint: "max".to_string(),
                                confidence: 0.9,
                                stop_loss: None,
                                take_profit: None,
                                reason: exit_reason,
                                timestamp_ms: ts,
                            });
                            position = None;
                            stop_loss = None;
                        }
                    } else if pos == "sell" {
                        // Exit on Stop Loss
                        if let Some(sl) = stop_loss {
                            if current_close_dec >= sl {
                                should_exit = true;
                                exit_reason = "Stop Loss hit".to_string();
                            }
                        }
                        // Exit condition: %K crosses above %D in oversold region
                        if !should_exit
                            && k_p <= d_p
                            && k_c > d_c
                            && k_c < self.config.oversold_threshold
                        {
                            should_exit = true;
                            exit_reason = format!(
                                "StochRSI %K ({:.2}) crossed above %D ({:.2}) below oversold threshold",
                                k_c, d_c
                            );
                        }

                        if should_exit {
                            signals.push(Signal {
                                signal_type: SignalType::Exit,
                                symbol: self.config.symbol.clone(),
                                side: "buy".to_string(),
                                size_hint: "max".to_string(),
                                confidence: 0.9,
                                stop_loss: None,
                                take_profit: None,
                                reason: exit_reason,
                                timestamp_ms: ts,
                            });
                            position = None;
                            stop_loss = None;
                        }
                    }
                }
            }
        }

        Ok(signals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stoch_rsi_empty_data() {
        let config = StochRsiMeanReversionConfig::default();
        let strategy = StochRsiMeanReversion::new(config);
        let df = DataFrame::default();
        let result = strategy.generate_signals(&df).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Data cannot be empty");
    }

    fn create_test_data() -> DataFrame {
        // Need rsi_period=3, stoch_period=3, k=3, d=3, atr=3
        let n = 20;
        let mut timestamps = Vec::with_capacity(n);
        let mut close = Vec::with_capacity(n);
        let mut high = Vec::with_capacity(n);
        let mut low = Vec::with_capacity(n);

        // First few values generic
        for i in 0..10 {
            timestamps.push(i as i64 * 60000);
            close.push(100.0 + i as f64);
            high.push(101.0 + i as f64);
            low.push(99.0 + i as f64);
        }

        // To trigger oversold long entry, we need %K < oversold_threshold and %K crosses above %D
        // RSI needs to drop sharply, then start rising.
        // Index 10-14: Drop sharply
        for i in 10..15 {
            timestamps.push(i as i64 * 60000);
            close.push(100.0 - (i - 10) as f64 * 5.0);
            high.push(close.last().unwrap() + 1.0);
            low.push(close.last().unwrap() - 1.0);
        }

        // Index 15-19: Rise sharply
        for i in 15..20 {
            timestamps.push(i as i64 * 60000);
            close.push(75.0 + (i - 15) as f64 * 5.0);
            high.push(close.last().unwrap() + 1.0);
            low.push(close.last().unwrap() - 1.0);
        }

        df!(
            "timestamp_unix_ms" => timestamps,
            "open" => close.clone(), // Doesn't matter
            "high" => high,
            "low" => low,
            "close" => close,
            "volume" => vec![1000.0; n]
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_stoch_rsi_signals() -> Result<()> {
        let config = StochRsiMeanReversionConfig {
            rsi_period: 3,
            stoch_period: 3,
            k_period: 3,
            d_period: 3,
            atr_period: 3,
            oversold_threshold: 40.0,
            overbought_threshold: 60.0,
            ..Default::default()
        };

        let strategy = StochRsiMeanReversion::new(config);
        let df = create_test_data();

        let signals = strategy.generate_signals(&df).await?;

        // Should have at least one entry signal
        assert!(!signals.is_empty(), "Expected at least one signal");

        Ok(())
    }
}
