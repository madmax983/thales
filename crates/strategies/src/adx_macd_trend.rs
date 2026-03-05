use crate::indicators::{adx, atr, macd};
use crate::strategy::{Signal, SignalType, Strategy, StrategyConfig, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

/// Configuration for the ADX + MACD Trend strategy.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AdxMacdTrendConfig {
    pub adx_period: usize,
    pub adx_threshold: f64,
    pub macd_fast_period: usize,
    pub macd_slow_period: usize,
    pub macd_signal_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for AdxMacdTrendConfig {}

/// ADX + MACD Trend Strategy
///
/// A trend-following strategy that combines the Average Directional Index (ADX)
/// for trend strength and Moving Average Convergence Divergence (MACD) for trend direction.
/// It enters long when the MACD Line crosses above the Signal Line, provided the ADX
/// indicates a strong trend (ADX > adx_threshold).
pub struct AdxMacdTrend {
    config: AdxMacdTrendConfig,
}

impl AdxMacdTrend {
    pub fn new(config: AdxMacdTrendConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for AdxMacdTrend {
    fn name(&self) -> &str {
        "AdxMacdTrend"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        if data.height() == 0 {
            return Ok(vec![]);
        }

        // Calculate MACD
        let (macd_line, signal_line, _hist) = macd::calculate(
            data,
            self.config.macd_fast_period,
            self.config.macd_slow_period,
            self.config.macd_signal_period,
        )?;

        // Calculate ADX
        let (adx_line, _plus_di, _minus_di) = adx::calculate(data, self.config.adx_period)?;

        // Calculate ATR
        let atr_series = atr::calculate(data, self.config.atr_period)?;

        let macd_f64 = macd_line.f64()?;
        let signal_f64 = signal_line.f64()?;
        let adx_f64 = adx_line.f64()?;
        let atr_f64 = atr_series.f64()?;

        let close_col = data.column("close")?.f64()?;
        let time_col = data.column("timestamp_unix_ms")?.i64()?;

        let mut signals = Vec::new();
        let mut in_long = false;
        let mut stop_loss = 0.0;

        for i in 1..data.height() {
            let prev_macd = macd_f64.get(i - 1);
            let curr_macd = macd_f64.get(i);
            let prev_signal = signal_f64.get(i - 1);
            let curr_signal = signal_f64.get(i);
            let curr_adx = adx_f64.get(i);
            let close = close_col.get(i);
            let atr_val = atr_f64.get(i);
            let timestamp = time_col.get(i).unwrap_or(0);

            if let (
                Some(p_macd),
                Some(c_macd),
                Some(p_sig),
                Some(c_sig),
                Some(c_adx),
                Some(price),
                Some(atr),
            ) = (
                prev_macd,
                curr_macd,
                prev_signal,
                curr_signal,
                curr_adx,
                close,
                atr_val,
            ) {
                if in_long {
                    // Check stop loss first
                    if price <= stop_loss {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 1.0,
                            stop_loss: None,
                            take_profit: None,
                            reason: "Stop Loss Hit".to_string(),
                            timestamp_ms: timestamp,
                        });
                        in_long = false;
                        continue;
                    }

                    // Exit Condition: MACD crosses BELOW Signal Line
                    if p_macd >= p_sig && c_macd < c_sig {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            stop_loss: None,
                            take_profit: None,
                            reason: "MACD Cross Down".to_string(),
                            timestamp_ms: timestamp,
                        });
                        in_long = false;
                    }
                } else {
                    // Entry Condition: MACD crosses ABOVE Signal Line AND ADX > threshold
                    if p_macd <= p_sig && c_macd > c_sig && c_adx > self.config.adx_threshold {
                        let calculated_sl = price - (atr * self.config.stop_loss_atr_mult);
                        signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.8,
                            stop_loss: Some(calculated_sl),
                            take_profit: None,
                            reason: "MACD Cross Up + Strong ADX".to_string(),
                            timestamp_ms: timestamp,
                        });
                        in_long = true;
                        stop_loss = calculated_sl;
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: AdxMacdTrendConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    fn create_test_data() -> DataFrame {
        let mut closes = vec![100.0; 100];
        let mut highs = vec![101.0; 100];
        let mut lows = vec![99.0; 100];
        let timestamps: Vec<i64> = (0..100).map(|i| i as i64 * 1000).collect();

        // 1. Initial flat/range period (0..30)
        // 2. Slow trend down to reset MACD below 0 (30..50)
        for i in 30..50 {
            closes[i] = closes[i - 1] - 0.5;
            highs[i] = closes[i] + 1.0;
            lows[i] = closes[i] - 1.0;
        }

        // 3. Sharp upward trend to trigger ADX rise and MACD crossover (50..100)
        for i in 50..100 {
            closes[i] = closes[i - 1] + 2.0;
            highs[i] = closes[i] + 2.5;
            lows[i] = closes[i] - 0.5;
        }

        df!(
            "timestamp_unix_ms" => timestamps,
            "open" => closes.clone(),
            "high" => highs,
            "low" => lows,
            "close" => closes,
            "volume" => vec![1000.0; 100]
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_empty_data() -> Result<()> {
        let config = AdxMacdTrendConfig {
            adx_period: 14,
            adx_threshold: 25.0,
            macd_fast_period: 12,
            macd_slow_period: 26,
            macd_signal_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };

        let strategy = AdxMacdTrend::new(config);
        let empty_df = DataFrame::empty();
        let signals = strategy.generate_signals(&empty_df).await?;
        assert!(signals.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_signal_generation() -> Result<()> {
        let config = AdxMacdTrendConfig {
            adx_period: 5,
            adx_threshold: 20.0,
            macd_fast_period: 5,
            macd_slow_period: 10,
            macd_signal_period: 3,
            stop_loss_atr_mult: 2.0,
            atr_period: 5,
            symbol: "TEST".to_string(),
        };

        let strategy = AdxMacdTrend::new(config);
        let df = create_test_data();
        let signals = strategy.generate_signals(&df).await?;

        // We should get at least one entry signal due to the strong synthetic trend.
        assert!(!signals.is_empty(), "Should generate signals on strong trend");

        let entry_signal = signals.iter().find(|s| s.signal_type == SignalType::Entry);
        assert!(entry_signal.is_some(), "Should have an entry signal");
        if let Some(s) = entry_signal {
            assert_eq!(s.side, "buy");
            assert!(s.stop_loss.is_some());
            assert_eq!(s.reason, "MACD Cross Up + Strong ADX");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_update_params() -> Result<()> {
        let mut strategy = AdxMacdTrend::new(AdxMacdTrendConfig {
            adx_period: 14,
            adx_threshold: 25.0,
            macd_fast_period: 12,
            macd_slow_period: 26,
            macd_signal_period: 9,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        });

        let new_params = serde_json::json!({
            "adx_period": 10,
            "adx_threshold": 30.0,
            "macd_fast_period": 5,
            "macd_slow_period": 10,
            "macd_signal_period": 3,
            "stop_loss_atr_mult": 1.5,
            "atr_period": 10,
            "symbol": "BTCUSD"
        });

        strategy.update_params(new_params).await?;

        assert_eq!(strategy.config.adx_period, 10);
        assert_eq!(strategy.config.adx_threshold, 30.0);
        assert_eq!(strategy.config.symbol, "BTCUSD");

        Ok(())
    }
}
