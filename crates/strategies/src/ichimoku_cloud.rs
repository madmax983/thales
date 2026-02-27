use crate::indicators::ichimoku;
use crate::strategy::{Signal, SignalType, Strategy, StrategyType};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;

pub struct IchimokuCloud {
    config: IchimokuCloudConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct IchimokuCloudConfig {
    pub tenkan_period: usize,
    pub kijun_period: usize,
    pub senkou_span_b_period: usize,
    pub senkou_span_offset: usize,
    pub chikou_span_offset: usize,
    pub symbol: String,
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

    fn strategy_type(&self) -> StrategyType {
        StrategyType::TrendFollowing
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        let (tenkan, kijun, span_a, span_b, _chikou) = ichimoku::calculate(
            data,
            self.config.tenkan_period,
            self.config.kijun_period,
            self.config.senkou_span_b_period,
            self.config.senkou_span_offset,
            self.config.chikou_span_offset,
        )?;

        let close = data.column("close")?.f64()?;
        let timestamp = data.column("timestamp_unix_ms")?.i64()?;

        let tenkan = tenkan.f64()?;
        let kijun = kijun.f64()?;
        let span_a = span_a.f64()?;
        let span_b = span_b.f64()?;

        let mut signals = Vec::new();
        let len = data.height();

        // Iterate through data to find crossovers
        // We need at least 1 previous bar for crossover check
        for i in 1..len {
            let ts = timestamp.get(i).unwrap_or(0);

            // Current Values
            let close_curr = close.get(i);
            let tenkan_curr = tenkan.get(i);
            let kijun_curr = kijun.get(i);
            let span_a_curr = span_a.get(i);
            let span_b_curr = span_b.get(i);

            // Previous Values
            let tenkan_prev = tenkan.get(i - 1);
            let kijun_prev = kijun.get(i - 1);

            // Ensure all necessary values are present
            if let (Some(c), Some(t), Some(k), Some(sa), Some(sb), Some(t_prev), Some(k_prev)) = (
                close_curr,
                tenkan_curr,
                kijun_curr,
                span_a_curr,
                span_b_curr,
                tenkan_prev,
                kijun_prev,
            ) {
                // Cloud Status
                // Bullish Cloud: Span A > Span B (Green)
                // Bearish Cloud: Span A < Span B (Red)
                // We check if Price is above BOTH spans (Above Cloud) or below BOTH spans (Below Cloud).
                let above_cloud = c > sa && c > sb;
                let below_cloud = c < sa && c < sb;

                // TK Cross
                // Bullish Cross: Tenkan crosses ABOVE Kijun
                let bullish_cross = t > k && t_prev <= k_prev;

                // Bearish Cross: Tenkan crosses BELOW Kijun
                let bearish_cross = t < k && t_prev >= k_prev;

                if bullish_cross && above_cloud {
                    // Strong Buy Signal
                    // Stop Loss: Kijun-sen
                    // Take Profit: Entry + 2 * (Entry - Kijun)
                    let sl = k; // Kijun is the SL
                    let risk = c - sl;
                    let tp = if risk > 0.0 {
                        Some(c + 2.0 * risk)
                    } else {
                        None
                    };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "buy".to_string(),
                        size_hint: "100".to_string(), // Default hint, will be sized by signals.rs
                        confidence: 0.8,              // High confidence for TK Cross above Cloud
                        stop_loss: Some(sl),
                        take_profit: tp,
                        reason: "Tenkan-Kijun Bullish Cross above Cloud".to_string(),
                        timestamp_ms: ts,
                    });
                } else if bearish_cross && below_cloud {
                    // Strong Sell Signal
                    // Stop Loss: Kijun-sen
                    // Take Profit: Entry - 2 * (Kijun - Entry)
                    let sl = k;
                    let risk = sl - c;
                    let tp = if risk > 0.0 {
                        Some(c - 2.0 * risk)
                    } else {
                        None
                    };

                    signals.push(Signal {
                        signal_type: SignalType::Entry,
                        symbol: self.config.symbol.clone(),
                        side: "sell".to_string(),
                        size_hint: "100".to_string(),
                        confidence: 0.8,
                        stop_loss: Some(sl),
                        take_profit: tp,
                        reason: "Tenkan-Kijun Bearish Cross below Cloud".to_string(),
                        timestamp_ms: ts,
                    });
                }
            }
        }

        Ok(signals)
    }

    async fn update_params(&mut self, _params: serde_json::Value) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_ichimoku_bullish_signal() -> Result<()> {
        let config = IchimokuCloudConfig {
            tenkan_period: 9,
            kijun_period: 26,
            senkou_span_b_period: 52,
            senkou_span_offset: 26,
            chikou_span_offset: 26,
            symbol: "TEST".to_string(),
        };
        let strategy = IchimokuCloud::new(config);

        // Construct scenario:
        // 1. Price above Cloud.
        // 2. Tenkan <= Kijun (prev) -> Tenkan > Kijun (curr).

        // We need enough data for indicators to be valid.
        // Max lookback is 52. Offset 26. So 78 points?
        // Let's mock the internal calculation or just provide enough data.
        // Actually, we can just feed a DataFrame where we know the indicator values will result in a signal.
        // But `ichimoku::calculate` re-calculates everything from OHLC.
        // So we need to construct OHLC that produces the desired indicator values.

        // Easier approach: Just 200 bars of "flat" data where Cloud is established low,
        // and then manipulate recent bars to cause Tenkan/Kijun cross.
        // We need > 78 bars for Cloud to be valid (Span B 52 + Shift 26).

        let n = 200;
        let mut closes = vec![100.0; n];
        let mut highs = vec![105.0; n];
        let mut lows = vec![95.0; n];
        let timestamps: Vec<i64> = (0..n).map(|i| i as i64 * 60000).collect();

        // 1. Establish Cloud below 100.
        // We need Price > Cloud. So let's raise price to 110 at the end, while Cloud stays lower (lagging).
        // Cloud at T is based on (T-26) data.
        // Let's set initial 150 bars to 90.
        for i in 0..150 {
            closes[i] = 90.0;
            highs[i] = 95.0;
            lows[i] = 85.0;
        }
        // Then jump to 110.
        for i in 150..n {
            closes[i] = 110.0;
            highs[i] = 115.0;
            lows[i] = 105.0;
        }

        // Now we need Tenkan crossing Kijun.
        // Tenkan (9) vs Kijun (26).
        // At index 99 (end), Tenkan looks at 90-99. Kijun looks at 74-99.
        // Both see high 115, low 105 (since index 60).
        // So Tenkan = (115+105)/2 = 110.
        // Kijun = (115+105)/2 = 110.
        // They are equal.

        // To make Tenkan Cross Above Kijun:
        // Tenkan needs to be Higher than Kijun.
        // Tenkan uses recent 9. Kijun uses recent 26.
        // If we have a higher High in the last 9 bars, Tenkan goes up.
        // But Kijun also sees it.
        // Unless... Kijun sees a LOWER Low in the 10-26 range that Tenkan doesn't see?
        // If Kijun includes older data which has lower Low, then Kijun might be LOWER?
        // No, (High + Low) / 2.
        // If Kijun sees a lower Low (e.g. 85 from index 50-60 range), its average might be lower.
        // Tenkan only sees 105 (from 90-99 range).

        // Let's verify:
        // At index 80 (20 bars after jump).
        // Tenkan (9): 71-80. All 110/115/105. -> 110.
        // Kijun (26): 54-80.
        //   54-59: Low 85. High 95.
        //   60-80: Low 105. High 115.
        //   Max High: 115. Min Low: 85.
        //   Kijun = (115 + 85) / 2 = 100.
        // So Tenkan (110) > Kijun (100).
        // And Cloud?
        // Cloud at 80 is based on 54. Price at 54 was 90. Cloud ~90.
        // Price at 80 is 110. 110 > 90. Above Cloud.
        // So we have Tenkan > Kijun AND Above Cloud.

        // But we need a CROSS.
        // Previous bar (79) needs Tenkan <= Kijun.
        // At 79:
        // Tenkan (9): 70-79. All 110/115/105. -> 110.
        // Kijun (26): 53-79. Max 115. Min 85. -> 100.
        // It's ALREADY crossed long ago (when price jumped).

        // We need to capture the MOMENT of cross.
        // The jump happened at 60.
        // At 60:
        // Tenkan (51-60). Includes jump. Max 115. Min 85 (from 51-59). -> 100.
        // Kijun (34-60). Max 115. Min 85. -> 100.
        // Equal?
        // At 61:
        // Tenkan (52-61). Min 85. Max 115. -> 100.
        // ...
        // Wait, Tenkan keeps seeing 85 until index 60+9 = 69.
        // At 69, Tenkan sees 60-69 (all high). Min 105. Max 115. -> 110.
        // Kijun still sees 85 until 60+26 = 86.
        // So at 69, Tenkan jumps to 110. Kijun stays at 100.
        // THIS is the cross!
        // Prev (68): Tenkan 100 (sees 85). Kijun 100. -> Tenkan <= Kijun.
        // Curr (69): Tenkan 110. Kijun 100. -> Tenkan > Kijun.
        // Signal at 69!

        let df = df!(
            "close" => &closes,
            "high" => &highs,
            "low" => &lows,
            "timestamp_unix_ms" => &timestamps
        )?;

        let signals = strategy.generate_signals(&df).await?;

        // We expect a signal around index 150 + 9 = 159.
        assert!(!signals.is_empty(), "Should generate signal");
        let signal = signals.last().unwrap(); // Get the last one (or search for specific one)

        assert_eq!(signal.side, "buy");
        assert!(signal.reason.contains("Bullish Cross"));
        // Kijun at 159 is 100. Close is 110. SL should be 100.
        assert_eq!(signal.stop_loss, Some(100.0));

        Ok(())
    }
}
