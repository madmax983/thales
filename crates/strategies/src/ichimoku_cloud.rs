use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{ichimoku, atr};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use async_trait::async_trait;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IchimokuCloudConfig {
    pub tenkan_period: usize,
    pub kijun_period: usize,
    pub senkou_b_period: usize,
    pub displacement: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}

impl StrategyConfig for IchimokuCloudConfig {}

pub struct IchimokuCloud {
    config: IchimokuCloudConfig,
}

impl IchimokuCloud {
    pub fn new(config: IchimokuCloudConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Strategy for IchimokuCloud {
    fn name(&self) -> &str {
        "IchimokuCloud"
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let close_series = data.column("close")?.clone();
        let high_series = data.column("high")?.clone();
        let low_series = data.column("low")?.clone();
        let time_series = data.column("timestamp_unix_ms")?;

        let close_arr = close_series.f64()?;
        let time_arr = time_series.cast(&DataType::Int64)?;
        let time_arr = time_arr.i64()?;

        // Calculate Ichimoku
        let ichimoku_out = ichimoku::calculate(
            &high_series,
            &low_series,
            &close_series,
            self.config.tenkan_period,
            self.config.kijun_period,
            self.config.senkou_b_period,
            self.config.displacement,
        )?;

        // Calculate ATR for Stop Loss
        let atr_series = atr::calculate(data, self.config.atr_period)?;
        let atr_arr = atr_series.f64()?;

        let tenkan = ichimoku_out.tenkan_sen.f64()?;
        let kijun = ichimoku_out.kijun_sen.f64()?;
        let senkou_a = ichimoku_out.senkou_span_a.f64()?;
        let senkou_b = ichimoku_out.senkou_span_b.f64()?;
        // Chikou Span is Close shifted back. Use Close vs Close[t-26] comparison manually if needed,
        // or just use the logic: Chikou > Price (at t-26).
        // Since we are iterating at 'i', we want to know if Chikou (which is Close[i]) is > Price[i-26].
        // This confirms the current price is higher than price 26 periods ago.

        let mut signals = Vec::new();
        let mut entry_price: Option<Decimal> = None;
        let mut position_side: Option<&str> = None; // "buy" or "sell"

        // We iterate. Start from max period to ensure valid values.
        let start_idx = self.config.senkou_b_period.max(self.config.displacement).max(26);

        for i in start_idx..close_arr.len() {
            let timestamp = time_arr.get(i).unwrap_or(0);
            let price_opt = close_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let atr_opt = atr_arr.get(i).and_then(|v| Decimal::from_f64_retain(v));

            let tenkan_curr = tenkan.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let kijun_curr = kijun.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let senkou_a_curr = senkou_a.get(i).and_then(|v| Decimal::from_f64_retain(v));
            let senkou_b_curr = senkou_b.get(i).and_then(|v| Decimal::from_f64_retain(v));

            // Previous values for crossover check
            let tenkan_prev = tenkan.get(i-1).and_then(|v| Decimal::from_f64_retain(v));
            let kijun_prev = kijun.get(i-1).and_then(|v| Decimal::from_f64_retain(v));

            // Chikou Check: Close[i] > Close[i - 26]
            let chikou_offset = self.config.displacement;
            let past_price_opt = if i >= chikou_offset {
                 close_arr.get(i - chikou_offset).and_then(|v| Decimal::from_f64_retain(v))
            } else {
                None
            };

            if let (Some(price), Some(t_c), Some(k_c), Some(sa_c), Some(sb_c), Some(atr), Some(t_p), Some(k_p), Some(past_p)) =
                (price_opt, tenkan_curr, kijun_curr, senkou_a_curr, senkou_b_curr, atr_opt, tenkan_prev, kijun_prev, past_price_opt)
            {
                let cloud_top = sa_c.max(sb_c);
                let cloud_bottom = sa_c.min(sb_c);
                let sl_pips = atr * Decimal::from_f64_retain(self.config.stop_loss_atr_mult).unwrap_or(Decimal::ONE);

                // Exit Logic
                if let Some(side) = position_side {
                    let mut exit_signal = false;
                    let mut reason = String::new();

                    if side == "buy" {
                        // Exit if Price drops below Kijun or Tenkan crosses below Kijun (weakness)
                        // Or Price drops below Cloud (reversal)
                        if price < k_c {
                            exit_signal = true;
                            reason = format!("Price closed below Kijun-sen ({:.2})", k_c);
                        } else if price < cloud_top {
                             exit_signal = true;
                             reason = "Price entered Cloud".to_string();
                        } else if t_c < k_c && t_p >= k_p {
                             // Bearish TK Cross
                             // exit_signal = true;
                             // reason = "Bearish TK Cross".to_string();
                             // Can be just a pullback. Let's stick to Price < Kijun for trend following exit.
                        }
                    } else { // sell
                         if price > k_c {
                            exit_signal = true;
                            reason = format!("Price closed above Kijun-sen ({:.2})", k_c);
                        } else if price > cloud_bottom {
                             exit_signal = true;
                             reason = "Price entered Cloud".to_string();
                        }
                    }

                    if exit_signal {
                        signals.push(Signal {
                            signal_type: SignalType::Exit,
                            symbol: self.config.symbol.clone(),
                            side: if side == "buy" { "sell".to_string() } else { "buy".to_string() },
                            size_hint: "max".to_string(),
                            confidence: 0.8,
                            stop_loss: None,
                            take_profit: None,
                            reason,
                            timestamp_ms: timestamp,
                        });
                        entry_price = None;
                        position_side = None;
                        continue;
                    }
                }

                // Entry Logic
                if entry_price.is_none() {
                    // LONG: Price > Cloud, TK Cross Bullish, Chikou > Price[t-26]
                    if price > cloud_top && t_c > k_c && t_p <= k_p && price > past_p {
                         let sl = price - sl_pips;
                         signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "buy".to_string(),
                            size_hint: "100".to_string(), // Sizing handled by signal generator
                            confidence: 0.9,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: None, // Trend following
                            reason: "Ichimoku Breakout: Price > Cloud, Bullish TK Cross, Chikou confirmation".to_string(),
                            timestamp_ms: timestamp,
                        });
                        entry_price = Some(price);
                        position_side = Some("buy");
                    }
                    // SHORT: Price < Cloud, TK Cross Bearish, Chikou < Price[t-26]
                    else if price < cloud_bottom && t_c < k_c && t_p >= k_p && price < past_p {
                        let sl = price + sl_pips;
                         signals.push(Signal {
                            signal_type: SignalType::Entry,
                            symbol: self.config.symbol.clone(),
                            side: "sell".to_string(),
                            size_hint: "100".to_string(),
                            confidence: 0.9,
                            stop_loss: Some(sl.to_f64().unwrap_or(0.0)),
                            take_profit: None,
                            reason: "Ichimoku Breakout: Price < Cloud, Bearish TK Cross, Chikou confirmation".to_string(),
                            timestamp_ms: timestamp,
                        });
                        entry_price = Some(price);
                        position_side = Some("sell");
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        let new_config: IchimokuCloudConfig = serde_json::from_value(params)?;
        self.config = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_ichimoku_signals() -> Result<()> {
        let config = IchimokuCloudConfig {
            tenkan_period: 9,
            kijun_period: 26,
            senkou_b_period: 52,
            displacement: 26,
            stop_loss_atr_mult: 2.0,
            atr_period: 14,
            symbol: "TEST".to_string(),
        };
        let strategy = IchimokuCloud::new(config);

        // Generate synthetic data for a Buy setup
        // 1. Flat market below cloud
        // 2. Rally above cloud
        // 3. TK Cross Bullish

        // We need substantial data for periods.
        let mut high = Vec::new();
        let mut low = Vec::new();
        let mut close = Vec::new();
        let mut timestamp = Vec::new();
        let start = 100000;

        // 100 bars
        // 0-70: Uptrend to establish Cloud
        for i in 0..70 {
            timestamp.push(start + i * 60000);
            let p = 100.0 + (i as f64 * 0.2); // Up to ~114
            high.push(p + 1.0);
            low.push(p - 1.0);
            close.push(p);
        }
        // 70-89: Downtrend/Pullback to cause Bearish TK Cross (T < K)
        // Highs must drop.
        // Max(26) will stay high (from index 70).
        // Max(9) will drop.
        for i in 70..90 {
            timestamp.push(start + i * 60000);
            let p = 114.0 - ((i - 70) as f64 * 0.5); // Drop to 104
            high.push(p + 1.0);
            low.push(p - 1.0);
            close.push(p);
        }

        // 90: Spike to cause Bullish TK Cross and Breakout
        timestamp.push(start + 90 * 60000);
        let spike_price = 150.0;
        high.push(spike_price + 5.0);
        low.push(spike_price - 2.0);
        close.push(spike_price);

        // Fill rest
        for i in 91..100 {
            timestamp.push(start + i * 60000);
            high.push(150.0);
            low.push(150.0);
            close.push(150.0);
        }

        // Ensure TK Cross Bullish.
        // Tenkan (9) needs to be > Kijun (26).
        // With a spike, Tenkan rises faster. So it should cross.

        // Also Chikou: Price[90] > Price[90-26]. 150 > 106. Yes.

        let df = df!(
            "timestamp_unix_ms" => timestamp,
            "high" => high,
            "low" => low,
            "close" => close
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // Should find at least one signal around index 90+
        assert!(!signals.is_empty(), "Should generate signals");

        let entry = &signals[0];
        assert_eq!(entry.signal_type, SignalType::Entry);
        assert_eq!(entry.side, "buy");
        assert!(entry.reason.contains("Ichimoku Breakout"));

        Ok(())
    }
}
