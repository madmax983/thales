use contracts::{Bar, BarSeries, MarketAnalysis};

pub fn analyze(series: &BarSeries) -> MarketAnalysis {
    let bars = &series.bars;
    let symbol = bars.first().map(|b| b.symbol.clone()).unwrap_or_default();
    let market = bars.first().map(|b| b.market.clone()).unwrap_or_default();
    let last_bar = bars.last();
    let timestamp = last_bar.map(|b| b.timestamp_unix_ms).unwrap_or(0);

    if bars.is_empty() {
        return MarketAnalysis {
            symbol,
            market,
            regime: "Unknown".to_string(),
            sentiment: "Neutral".to_string(),
            patterns: vec![],
            key_levels: vec![],
            volatility: "Unknown".to_string(),
            confidence: 0.0,
            timestamp_unix_ms: timestamp,
        };
    }

    let regime = calculate_regime(bars);
    let volatility = calculate_volatility(bars);
    let patterns = detect_patterns(bars);
    let key_levels = identify_levels(bars);

    let sentiment = calculate_sentiment(bars, &regime, &patterns);

    MarketAnalysis {
        symbol,
        market,
        regime,
        sentiment,
        patterns,
        key_levels,
        volatility,
        confidence: 0.8,
        timestamp_unix_ms: timestamp,
    }
}

pub fn log_signal_to_file(analysis: &MarketAnalysis, path: &str) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    writeln!(file, "\n## Symbol: {} ({})", analysis.symbol, analysis.market)?;
    writeln!(file, "**Timestamp**: {}", analysis.timestamp_unix_ms)?;
    writeln!(file, "\n### Analysis Summary")?;
    writeln!(file, "- **Regime**: {}", analysis.regime)?;
    writeln!(file, "- **Sentiment**: {}", analysis.sentiment)?;
    writeln!(file, "- **Volatility**: {}", analysis.volatility)?;
    writeln!(file, "- **Confidence**: {:.0}%", analysis.confidence * 100.0)?;

    writeln!(file, "\n### Key Levels")?;
    if analysis.key_levels.len() >= 2 {
        writeln!(file, "- **Resistance**: {:.2}", analysis.key_levels[0])?;
        writeln!(file, "- **Support**: {:.2}", analysis.key_levels[1])?;
    } else {
         writeln!(file, "- None detected")?;
    }

    writeln!(file, "\n### Patterns Detected")?;
    if analysis.patterns.is_empty() {
        writeln!(file, "- None")?;
    } else {
        for p in &analysis.patterns {
            writeln!(file, "- {}", p)?;
        }
    }

    writeln!(file, "\n### Raw Analysis Data")?;
    let json = serde_json::to_string(analysis).unwrap_or_default();
    writeln!(file, "```json\n{}\n```", json)?;

    Ok(())
}

fn calculate_regime(bars: &[Bar]) -> String {
    if bars.len() < 50 {
        // Fallback to simple comparison if not enough data for SMA50
        if bars.len() < 10 {
            return "Unknown".to_string();
        }
        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let sma10 = calculate_sma(&closes, 10).unwrap_or(closes.last().unwrap().clone());
        let last = closes.last().unwrap();
        if *last > sma10 {
            return "Trending Up".to_string();
        } else {
            return "Trending Down".to_string();
        }
    }

    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let sma20 = calculate_sma_series(&closes, 20);
    let sma50 = calculate_sma_series(&closes, 50);

    match (sma20.last(), sma50.last()) {
        (Some(s20), Some(s50)) => {
            if s20 > s50 {
                // Check slope of SMA20
                let len = sma20.len();
                if len >= 2 && sma20[len-1] > sma20[len-2] {
                     "Trending Up".to_string()
                } else {
                     "Ranging".to_string() // Weak uptrend or ranging
                }
            } else {
                let len = sma20.len();
                if len >= 2 && sma20[len-1] < sma20[len-2] {
                    "Trending Down".to_string()
                } else {
                    "Ranging".to_string()
                }
            }
        },
        _ => "Unknown".to_string(),
    }
}

fn calculate_volatility(bars: &[Bar]) -> String {
    if bars.len() < 14 {
        return "Unknown".to_string();
    }
    let atr = calculate_atr(bars, 14).unwrap_or(0.0);
    let last_close = bars.last().unwrap().close;

    if last_close == 0.0 { return "Unknown".to_string(); }

    let rel_vol = atr / last_close;

    if rel_vol > 0.03 {
        "High".to_string()
    } else if rel_vol > 0.01 {
        "Medium".to_string()
    } else {
        "Low".to_string()
    }
}

fn calculate_sentiment(bars: &[Bar], regime: &str, patterns: &[String]) -> String {
    // Base sentiment on regime
    let mut score = match regime {
        "Trending Up" => 1,
        "Trending Down" => -1,
        _ => 0,
    };

    // Adjust based on patterns
    for p in patterns {
        match p.as_str() {
            "Hammer" | "Bullish Engulfing" | "Morning Star" => score += 1,
            "Shooting Star" | "Bearish Engulfing" | "Evening Star" => score -= 1,
            _ => {},
        }
    }

    // Adjust based on recent momentum (RSI proxy: closes > opens in last 3 bars)
    if bars.len() >= 3 {
        let bulls = bars.iter().rev().take(3).filter(|b| b.close > b.open).count();
        if bulls == 3 { score += 1; }
        else if bulls == 0 { score -= 1; }
    }

    if score >= 2 { "Strong Bullish".to_string() }
    else if score == 1 { "Bullish".to_string() }
    else if score == 0 { "Neutral".to_string() }
    else if score == -1 { "Bearish".to_string() }
    else { "Strong Bearish".to_string() }
}

fn detect_patterns(bars: &[Bar]) -> Vec<String> {
    let mut patterns = Vec::new();
    if bars.len() < 2 { return patterns; }

    let curr = &bars[bars.len()-1];
    let prev = &bars[bars.len()-2];

    let curr_body = (curr.close - curr.open).abs();
    let _prev_body = (prev.close - prev.open).abs();
    let curr_range = curr.high - curr.low;

    // Hammer / Shooting Star
    let upper_wick = curr.high - curr.close.max(curr.open);
    let lower_wick = curr.close.min(curr.open) - curr.low;

    if lower_wick > curr_body * 2.0 && upper_wick < curr_body * 0.5 && curr_range > 0.0 {
        patterns.push("Hammer".to_string());
    }
    if upper_wick > curr_body * 2.0 && lower_wick < curr_body * 0.5 && curr_range > 0.0 {
        patterns.push("Shooting Star".to_string());
    }

    // Engulfing
    let curr_bullish = curr.close > curr.open;
    let prev_bullish = prev.close > prev.open;

    if curr_bullish && !prev_bullish {
        if curr.close > prev.open && curr.open < prev.close {
             patterns.push("Bullish Engulfing".to_string());
        }
    } else if !curr_bullish && prev_bullish {
        if curr.close < prev.open && curr.open > prev.close {
             patterns.push("Bearish Engulfing".to_string());
        }
    }

    // Doji
    if curr_body <= curr_range * 0.1 && curr_range > 0.0 {
        patterns.push("Doji".to_string());
    }

    patterns
}

fn identify_levels(bars: &[Bar]) -> Vec<f64> {
    if bars.len() < 20 {
        return vec![];
    }
    // Simple Swing Highs/Lows over window of 20
    let window = &bars[bars.len().saturating_sub(20)..];
    let highest = window.iter().map(|b| b.high).fold(f64::NEG_INFINITY, |a, b| a.max(b));
    let lowest = window.iter().map(|b| b.low).fold(f64::INFINITY, |a, b| a.min(b));

    // Check if these are somewhat significant (e.g. not just the last bar)
    // For now, just return them as support/resistance
    vec![highest, lowest]
}

// --- Helpers ---

fn calculate_sma(data: &[f64], period: usize) -> Option<f64> {
    if data.len() < period { return None; }
    let sum: f64 = data.iter().rev().take(period).sum();
    Some(sum / period as f64)
}

fn calculate_sma_series(data: &[f64], period: usize) -> Vec<f64> {
    let mut smas = Vec::new();
    if data.len() < period { return smas; }

    let mut sum: f64 = data.iter().take(period).sum();
    smas.push(sum / period as f64);

    for i in period..data.len() {
        sum += data[i] - data[i-period];
        smas.push(sum / period as f64);
    }
    smas
}

fn calculate_tr(high: f64, low: f64, prev_close: f64) -> f64 {
    let hl = high - low;
    let hpc = (high - prev_close).abs();
    let lpc = (low - prev_close).abs();
    hl.max(hpc).max(lpc)
}

fn calculate_atr(bars: &[Bar], period: usize) -> Option<f64> {
    if bars.len() < period + 1 { return None; }

    let mut trs = Vec::new();
    for i in 1..bars.len() {
        let tr = calculate_tr(bars[i].high, bars[i].low, bars[i-1].close);
        trs.push(tr);
    }

    // First ATR is simple average of TRs
    if trs.len() < period { return None; }

    let first_atr_sum: f64 = trs.iter().take(period).sum();
    let mut atr = first_atr_sum / period as f64;

    // Subsequent ATRs: (Previous ATR * (n-1) + Current TR) / n
    for i in period..trs.len() {
        atr = (atr * (period as f64 - 1.0) + trs[i]) / period as f64;
    }

    Some(atr)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_bars(prices: &[f64]) -> BarSeries {
        let mut bars = Vec::new();
        let mut time = 1000;
        for &p in prices {
            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "crypto".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: time,
                open: p,
                high: p + 1.0,
                low: p - 1.0,
                close: p,
                volume: 100.0,
            });
            time += 60000;
        }
        BarSeries {
            schema_version: "v0".to_string(),
            bars,
        }
    }

    #[test]
    fn test_sma_calculation() {
        let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        assert_eq!(calculate_sma(&data, 3), Some(40.0)); // (30+40+50)/3
        assert_eq!(calculate_sma(&data, 5), Some(30.0)); // (10+20+30+40+50)/5
        assert_eq!(calculate_sma(&data, 6), None);
    }

    #[test]
    fn test_regime_uptrend() {
        // Create an uptrend series
        let mut prices = Vec::new();
        for i in 0..60 {
            prices.push(100.0 + i as f64);
        }
        let series = create_mock_bars(&prices);
        let analysis = analyze(&series);
        assert_eq!(analysis.regime, "Trending Up");
    }

    #[test]
    fn test_regime_downtrend() {
        // Create a downtrend series
        let mut prices = Vec::new();
        for i in 0..60 {
            prices.push(200.0 - i as f64);
        }
        let series = create_mock_bars(&prices);
        let analysis = analyze(&series);
        assert_eq!(analysis.regime, "Trending Down");
    }

    #[test]
    fn test_pattern_detection() {
         // Create a Hammer
         let bar = Bar {
             symbol: "TEST".to_string(),
             market: "crypto".to_string(),
             timeframe: "1m".to_string(),
             timestamp_unix_ms: 1000,
             open: 100.0,
             high: 100.1,
             low: 95.0,
             close: 100.1,
             volume: 100.0,
         };
         // Need at least 2 bars
         let prev = Bar {
             symbol: "TEST".to_string(),
             market: "crypto".to_string(),
             timeframe: "1m".to_string(),
             timestamp_unix_ms: 0,
             open: 100.0,
             high: 105.0,
             low: 95.0,
             close: 100.0,
             volume: 100.0,
         };

         let series = BarSeries {
             schema_version: "v0".to_string(),
             bars: vec![prev, bar],
         };

         let analysis = analyze(&series);
         assert!(analysis.patterns.contains(&"Hammer".to_string()));
    }
}
