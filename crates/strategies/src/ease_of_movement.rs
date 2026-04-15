//! The Ease of Movement (EOM) Strategy
//!
//! Uses the EOM indicator to find trends with strong volume support.
//!
use crate::indicators::{atr, eom, sma};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Configuration parameters for the `EaseOfMovement` strategy.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EaseOfMovementConfig {
    /// The lookback period for the Ease of Movement indicator.
    pub eom_period: usize,
    /// The period for the SMA of the EOM (Signal Line).
    pub sma_period: usize,
    /// The multiplier applied to the ATR to calculate the trailing stop loss distance.
    pub stop_loss_atr_mult: f64,
    /// The lookback period for calculating the Average True Range (ATR).
    pub atr_period: usize,
    /// The market symbol this strategy is targeting (e.g., "BTCUSD").
    pub symbol: String,
}

impl Default for EaseOfMovementConfig {
    fn default() -> Self {
        Self {
            eom_period: 14,
            sma_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        }
    }
}

impl StrategyConfig for EaseOfMovementConfig {}

/// A momentum and volume-based strategy using the Ease of Movement (EOM) indicator.
///
/// It enters long when the EOM line crosses above its Simple Moving Average (SMA),
/// and enters short when the EOM line crosses below its SMA.
///
/// # Examples
///
/// ```rust
/// use strategies::ease_of_movement::{EaseOfMovement, EaseOfMovementConfig};
///
/// let config = EaseOfMovementConfig {
///     eom_period: 14,
///     sma_period: 9,
///     stop_loss_atr_mult: 2.0,
///     atr_period: 14,
///     symbol: "BTCUSD".to_string(),
/// };
///
/// let strategy = EaseOfMovement::new(config).unwrap();
/// ```
pub struct EaseOfMovement {
    config: EaseOfMovementConfig,
}

impl EaseOfMovement {
    pub fn new(config: EaseOfMovementConfig) -> Result<Self> {
        if config.eom_period == 0 || config.sma_period == 0 {
            anyhow::bail!("Periods must be greater than 0");
        }
        Ok(Self { config })
    }
}

#[async_trait]
impl Strategy for EaseOfMovement {
    fn name(&self) -> &str {
        "EaseOfMovement"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() < self.config.eom_period + self.config.sma_period + self.config.atr_period
        {
            return Ok(vec![]);
        }

        // Calculate EOM
        let eom_series = eom::calculate(data, self.config.eom_period)?;
        let eom_arr = eom_series.f64()?;

        // Calculate SMA of EOM (Signal line)
        // We have to put EOM in a dataframe for SMA to read the "close" column,
        // or we can pass a temp DF. Our SMA uses "close" so let's rename eom to close
        let mut eom_cloned = eom_series.clone();
        eom_cloned.rename("close");
        let temp_df = DataFrame::new(vec![eom_cloned])?;
        let signal_series = sma::calculate(&temp_df, self.config.sma_period)?;
        let signal_arr = signal_series.f64()?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let close_series = data.column("close")?.f64()?;
        let time_series = data.column("timestamp_unix_ms")?.cast(&DataType::Int64)?;
        let time_arr = time_series.i64()?;

        let mut signals = Vec::new();
        let sl_mult =
            Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..data.height() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let eom_curr = eom_arr.get(i);
            let eom_prev = eom_arr.get(i - 1);

            let sig_curr = signal_arr.get(i);
            let sig_prev = signal_arr.get(i - 1);

            let price_opt = close_series.get(i);
            let atr_opt = atr_arr.get(i);

            if let (
                Some(e_curr),
                Some(e_prev),
                Some(s_curr),
                Some(s_prev),
                Some(price),
                Some(atr_val),
            ) = (eom_curr, eom_prev, sig_curr, sig_prev, price_opt, atr_opt)
            {
                let price_dec = Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                // Entry Long: EOM crosses above SMA
                if e_prev <= s_prev && e_curr > s_curr {
                    // Exit any existing Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Buy to cover short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "EOM crossed above Signal Line (Trend Reversal Up)".to_string(),
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
                        reason: "EOM crossed above Signal Line (Bullish Momentum)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
                // Entry Short: EOM crosses below SMA
                else if e_prev >= s_prev && e_curr < s_curr {
                    // Exit any existing Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Sell to close long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: "EOM crossed below Signal Line (Trend Reversal Down)".to_string(),
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
                        reason: "EOM crossed below Signal Line (Bearish Momentum)".to_string(),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: EaseOfMovementConfig = serde_json::from_value(params)?;
        if new_config.eom_period == 0 || new_config.sma_period == 0 {
            anyhow::bail!("Periods must be greater than 0");
        }
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_eom_signal_generation() -> Result<()> {
        let config = EaseOfMovementConfig {
            eom_period: 2,
            sma_period: 2,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };

        let strategy = EaseOfMovement::new(config)?;

        let mut high_vals = vec![];
        let mut low_vals = vec![];
        let mut close_vals = vec![];
        let mut vol_vals = vec![];
        let mut ts_vals = vec![];

        let mut price = 100.0;

        // Generate data. Need to cross EOM SMA.
        for i in 0..20 {
            let vol = if i < 10 {
                // Moving up with high volume -> positive EOM
                price += 2.0;
                200_000_000.0
            } else {
                // Moving down with high volume -> negative EOM
                price -= 2.0;
                200_000_000.0
            };
            high_vals.push(price + 1.0);
            low_vals.push(price - 1.0);
            close_vals.push(price);
            vol_vals.push(vol);
            ts_vals.push(i as i64 * 1000);
        }

        let df = df!(
            "timestamp_unix_ms" => ts_vals,
            "open" => close_vals.clone(),
            "high" => high_vals,
            "low" => low_vals,
            "close" => close_vals,
            "volume" => vol_vals
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Should find at least one entry signal because trend changes
        assert!(!signals.is_empty(), "Should generate signals");

        let entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry.is_some(), "Should find entry signal");

        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        let mut strategy = EaseOfMovement::new(EaseOfMovementConfig {
            eom_period: 14,
            sma_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        })?;

        // Valid update
        let new_params = serde_json::json!({
            "eom_period": 20,
            "sma_period": 10,
            "stop_loss_atr_mult": 3.0,
            "atr_period": 14,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;
        assert_eq!(strategy.config.eom_period, 20);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        // Invalid update (0 period)
        let invalid_params = serde_json::json!({
            "eom_period": 0,
            "sma_period": 10,
            "stop_loss_atr_mult": 2.0,
            "atr_period": 14,
            "symbol": "TEST"
        });

        let res = strategy.update_params(invalid_params).await;
        assert!(res.is_err());

        Ok(())
    }
}
