use crate::indicators::{atr, kama, rsi};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Configuration for the KamaTrendFollowing strategy.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct KamaTrendFollowingConfig {
    /// KAMA efficiency ratio lookback period
    pub kama_period: usize,
    /// Fast EMA period for KAMA calculation
    pub kama_fast_period: usize,
    /// Slow EMA period for KAMA calculation
    pub kama_slow_period: usize,
    /// Lookback period for RSI
    pub rsi_period: usize,
    /// RSI threshold for entering a long position (e.g. 50.0)
    pub rsi_buy_threshold: f64,
    /// RSI threshold for entering a short position (e.g. 50.0)
    pub rsi_sell_threshold: f64,
    /// Stop-loss multiplier based on ATR
    pub stop_loss_atr_mult: f64,
    /// ATR period for stop-loss and take-profit calculations
    pub atr_period: usize,
    /// The symbol to trade
    pub symbol: String,
}

impl StrategyConfig for KamaTrendFollowingConfig {}

impl KamaTrendFollowingConfig {
    /// Validates the configuration parameters.
    pub fn validate(&self) -> Result<()> {
        if self.kama_period == 0 {
            anyhow::bail!("kama_period must be > 0");
        }
        if self.kama_fast_period == 0 {
            anyhow::bail!("kama_fast_period must be > 0");
        }
        if self.kama_slow_period == 0 {
            anyhow::bail!("kama_slow_period must be > 0");
        }
        if self.kama_fast_period >= self.kama_slow_period {
            anyhow::bail!("kama_fast_period must be strictly less than kama_slow_period");
        }
        if self.rsi_period == 0 {
            anyhow::bail!("rsi_period must be > 0");
        }
        if self.rsi_buy_threshold < 0.0 || self.rsi_buy_threshold > 100.0 {
            anyhow::bail!("rsi_buy_threshold must be between 0.0 and 100.0");
        }
        if self.rsi_sell_threshold < 0.0 || self.rsi_sell_threshold > 100.0 {
            anyhow::bail!("rsi_sell_threshold must be between 0.0 and 100.0");
        }
        if self.atr_period == 0 {
            anyhow::bail!("atr_period must be > 0");
        }
        if self.stop_loss_atr_mult <= 0.0 {
            anyhow::bail!("stop_loss_atr_mult must be > 0.0");
        }
        Ok(())
    }
}

/// KamaTrendFollowing Strategy
///
/// This strategy uses Kaufman's Adaptive Moving Average (KAMA) and Relative Strength Index (RSI).
///
/// **Entry Conditions:**
/// - **Long Entry:** Price closes ABOVE KAMA and RSI > `rsi_buy_threshold`.
/// - **Short Entry:** Price closes BELOW KAMA and RSI < `rsi_sell_threshold`.
///
/// **Exit Conditions:**
/// - A long position is exited if a short entry condition is met (or stop loss/take profit hit).
/// - A short position is exited if a long entry condition is met (or stop loss/take profit hit).
///
/// **Position Sizing and Risk Management:**
/// - Sizing is fixed (e.g. "100" units).
/// - Stop-loss is dynamically calculated using ATR (`stop_loss_atr_mult`).
/// - Take-profit is set at a 1:2 Risk/Reward ratio.
pub struct KamaTrendFollowing {
    pub config: KamaTrendFollowingConfig,
}

impl KamaTrendFollowing {
    /// Creates a new KamaTrendFollowing strategy with the given configuration.
    pub fn new(config: KamaTrendFollowingConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for KamaTrendFollowing {
    fn name(&self) -> &str {
        "KamaTrendFollowing"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        self.config.validate()?;

        if data.height() < self.config.kama_period.max(self.config.rsi_period) + 1 {
            return Ok(vec![]);
        }

        let time_series = data.column("timestamp_unix_ms")?;
        let time_arr = time_series.i64()?;

        let close_series = data.column("close")?;
        let close_arr = close_series.f64()?;

        let kama_series = kama::calculate(
            data,
            self.config.kama_period,
            self.config.kama_fast_period,
            self.config.kama_slow_period,
        )?;
        let kama_arr = kama_series.f64()?;

        let rsi_series = rsi::calculate(data, self.config.rsi_period)?;
        let rsi_arr = rsi_series.f64()?;

        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let mut signals = Vec::new();
        let sl_mult = Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ZERO);
        let two_dec = Decimal::from(2);

        for i in 1..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);

            let price_c = close_arr.get(i);
            let price_p = close_arr.get(i - 1);
            let kama_c = kama_arr.get(i);
            let kama_p = kama_arr.get(i - 1);
            let rsi_c = rsi_arr.get(i);
            let atr_c = atr_arr.get(i);

            if let (Some(pc), Some(pp), Some(kc), Some(kp), Some(rc), Some(ac)) =
                (price_c, price_p, kama_c, kama_p, rsi_c, atr_c)
            {
                let price_dec = Decimal::from_f64_retain(pc).unwrap_or(Decimal::ZERO);
                let atr_dec = Decimal::from_f64_retain(ac).unwrap_or(Decimal::ZERO);

                // Long Entry: Price crosses ABOVE KAMA and RSI > Buy Threshold
                if pp <= kp && pc > kc && rc > self.config.rsi_buy_threshold {
                    // Exit any existing Short
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Exit Short
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bullish Crossover: Price {:.2} > KAMA {:.2} & RSI {:.2} > {:.2}",
                            pc, kc, rc, self.config.rsi_buy_threshold
                        ),
                        timestamp_ms: timestamp,
                    });

                    // Enter Long
                    let sl = price_dec - (atr_dec * sl_mult);
                    let risk = price_dec - sl;
                    let tp = price_dec + (risk * two_dec);

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(), // Entry Long
                        size_hint: "100".to_string(), // Fixed position size hint
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "Bullish Crossover: Price {:.2} > KAMA {:.2} & RSI {:.2} > {:.2}",
                            pc, kc, rc, self.config.rsi_buy_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
                // Short Entry: Price crosses BELOW KAMA and RSI < Sell Threshold
                else if pp >= kp && pc < kc && rc < self.config.rsi_sell_threshold {
                    // Exit any existing Long
                    signals.push(Signal {
                        signal_type: SignalType::Exit,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(), // Exit Long
                        size_hint: "max".to_string(),
                        confidence: 0.8,
                        stop_loss: None,
                        take_profit: None,
                        reason: format!(
                            "Bearish Crossover: Price {:.2} < KAMA {:.2} & RSI {:.2} < {:.2}",
                            pc, kc, rc, self.config.rsi_sell_threshold
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
                        size_hint: "100".to_string(), // Fixed position size hint
                        confidence: 0.8,
                        stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                        take_profit: Some(tp.to_f64().unwrap_or(0.0)),
                        reason: format!(
                            "Bearish Crossover: Price {:.2} < KAMA {:.2} & RSI {:.2} < {:.2}",
                            pc, kc, rc, self.config.rsi_sell_threshold
                        ),
                        timestamp_ms: timestamp,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: KamaTrendFollowingConfig = serde_json::from_value(params)?;
        new_config.validate()?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_kama_trend_following_signals() -> Result<()> {
        let config = KamaTrendFollowingConfig {
            kama_period: 2,
            kama_fast_period: 2,
            kama_slow_period: 30,
            rsi_period: 2,
            rsi_buy_threshold: 50.0,
            rsi_sell_threshold: 50.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = KamaTrendFollowing::new(config);

        let timestamps = vec![1000i64, 2000, 3000, 4000, 5000, 6000];
        // Prices constructed to force KAMA crossover AND specific RSI direction
        // To get RSI > 50, price needs to go up strongly.
        // To trigger entry, price needs to go from below KAMA to above KAMA.
        let closes = vec![10.0, 10.0, 10.0, 15.0, 15.0, 10.0];
        let highs = vec![10.0, 10.0, 10.0, 16.0, 15.0, 11.0];
        let lows = vec![10.0, 10.0, 10.0, 10.0, 14.0, 9.0];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "open" => closes.clone(),
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Expect Entry at some point due to price shooting up
        assert!(!signals.is_empty(), "Should generate signals");

        let entry = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_kama_trend_following_short_signals() -> Result<()> {
        let config = KamaTrendFollowingConfig {
            kama_period: 2,
            kama_fast_period: 2,
            kama_slow_period: 30,
            rsi_period: 2,
            rsi_buy_threshold: 50.0,
            rsi_sell_threshold: 50.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 2,
            symbol: "TEST".to_string(),
        };
        let strategy = KamaTrendFollowing::new(config);

        let timestamps = vec![1000i64, 2000, 3000, 4000, 5000, 6000];
        // Prices constructed to force KAMA cross below AND RSI < 50
        let closes = vec![20.0, 20.0, 20.0, 10.0, 10.0, 15.0];
        let highs = vec![20.0, 20.0, 20.0, 11.0, 11.0, 16.0];
        let lows = vec![20.0, 20.0, 20.0, 9.0, 9.0, 14.0];

        let df = df!(
            "timestamp_unix_ms" => timestamps,
            "open" => closes.clone(),
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let signals = strategy.generate_signals(&df).await?;

        assert!(!signals.is_empty(), "Should generate signals");

        let entry = signals
            .iter()
            .find(|s| s.signal_type == SignalType::Entry && s.side == "sell");
        assert!(entry.is_some());

        Ok(())
    }

    #[test]
    fn test_kama_trend_following_validation() {
        let mut config = KamaTrendFollowingConfig {
            kama_period: 10,
            kama_fast_period: 2,
            kama_slow_period: 30,
            rsi_period: 14,
            rsi_buy_threshold: 50.0,
            rsi_sell_threshold: 50.0,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        assert!(config.validate().is_ok());

        config.kama_fast_period = 30;
        config.kama_slow_period = 30;
        assert!(config.validate().is_err(), "fast >= slow should fail");

        config.kama_fast_period = 2;
        config.rsi_buy_threshold = 150.0;
        assert!(config.validate().is_err(), "rsi > 100 should fail");
    }
}
