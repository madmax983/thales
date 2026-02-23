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
    let sentiment = calculate_sentiment(bars, &regime);

    let mut confidence: f64 = 0.5;
    if regime == "Trending Up" || regime == "Trending Down" {
        confidence += 0.2;
    }
    if (regime == "Trending Up" && sentiment.contains("Bullish"))
        || (regime == "Trending Down" && sentiment.contains("Bearish"))
    {
        confidence += 0.1;
    }
    for pattern in &patterns {
        if (regime == "Trending Up" && (pattern.contains("Bullish") || pattern == "Hammer"))
            || (regime == "Trending Down"
                && (pattern.contains("Bearish") || pattern == "Shooting Star"))
        {
            confidence += 0.1;
            break;
        }
    }
    confidence = confidence.min(1.0);

    MarketAnalysis {
        symbol,
        market,
        regime,
        sentiment,
        patterns,
        key_levels,
        volatility,
        confidence,
        timestamp_unix_ms: timestamp,
    }
}

fn calculate_regime(bars: &[Bar]) -> String {
    if bars.len() < 50 {
        // Fallback for short history
        if bars.len() < 20 {
             return "Unknown".to_string();
        }
        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let sma20 = calculate_sma(&closes, 20);
        let last_close = closes.last().unwrap();

        if let Some(sma) = sma20 {
            if *last_close > sma * 1.01 {
                return "Trending Up".to_string();
            } else if *last_close < sma * 0.99 {
                return "Trending Down".to_string();
            }
        }
        return "Ranging".to_string();
    }

    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let sma20 = calculate_sma(&closes, 20);
    let sma50 = calculate_sma(&closes, 50);

    match (sma20, sma50) {
        (Some(s20), Some(s50)) => {
            if s20 > s50 * 1.005 { // 0.5% buffer
                "Trending Up".to_string()
            } else if s20 < s50 * 0.995 {
                "Trending Down".to_string()
            } else {
                "Ranging".to_string()
            }
        }
        _ => "Unknown".to_string(),
    }
}

fn calculate_volatility(bars: &[Bar]) -> String {
    let atr = calculate_atr(bars, 14);
    let last_close = bars.last().map(|b| b.close).unwrap_or(1.0);

    match atr {
        Some(val) => {
            let ratio = val / last_close;
            if ratio > 0.02 {
                "High".to_string()
            } else if ratio > 0.01 {
                "Medium".to_string()
            } else {
                "Low".to_string()
            }
        }
        None => "Unknown".to_string(),
    }
}

fn calculate_sentiment(bars: &[Bar], regime: &str) -> String {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let rsi = calculate_rsi(&closes, 14);

    if let Some(val) = rsi {
        if val > 70.0 {
            return "Bullish (Overbought)".to_string();
        } else if val < 30.0 {
            return "Bearish (Oversold)".to_string();
        } else if val > 55.0 {
            return "Bullish".to_string();
        } else if val < 45.0 {
            return "Bearish".to_string();
        }
    }

    match regime {
        "Trending Up" => "Bullish".to_string(),
        "Trending Down" => "Bearish".to_string(),
        _ => "Neutral".to_string(),
    }
}

fn detect_patterns(bars: &[Bar]) -> Vec<String> {
    let mut patterns = Vec::new();
    if bars.len() < 2 {
        return patterns;
    }

    let curr = &bars[bars.len() - 1];
    let prev = &bars[bars.len() - 2];

    let curr_body = (curr.close - curr.open).abs();
    let curr_range = curr.high - curr.low;
    let prev_body = (prev.close - prev.open).abs();

    // Hammer / Shooting Star (Single candle)
    let upper_wick = curr.high - curr.close.max(curr.open);
    let lower_wick = curr.close.min(curr.open) - curr.low;

    if curr_range > 0.0 {
        if lower_wick > curr_body * 2.0 && upper_wick < curr_body * 0.5 {
            patterns.push("Hammer".to_string());
        }
        if upper_wick > curr_body * 2.0 && lower_wick < curr_body * 0.5 {
            patterns.push("Shooting Star".to_string());
        }
        // Doji
        if curr_body <= curr_range * 0.1 {
             patterns.push("Doji".to_string());
        }
    }

    // Engulfing (Two candle)
    let prev_is_red = prev.close < prev.open;
    let curr_is_green = curr.close > curr.open;

    if prev_is_red && curr_is_green {
         if curr.open <= prev.close && curr.close >= prev.open && curr_body > prev_body {
             patterns.push("Bullish Engulfing".to_string());
         }
    }

    let prev_is_green = prev.close > prev.open;
    let curr_is_red = curr.close < curr.open;

    if prev_is_green && curr_is_red {
        if curr.open >= prev.close && curr.close <= prev.open && curr_body > prev_body {
            patterns.push("Bearish Engulfing".to_string());
        }
    }

    patterns
}

fn identify_levels(bars: &[Bar]) -> Vec<f64> {
    let mut levels = Vec::new();
    if bars.len() < 10 {
        return levels;
    }

    let w20 = &bars[bars.len().saturating_sub(20)..];
    let h20 = w20.iter().map(|b| b.high).fold(f64::NEG_INFINITY, |a, b| a.max(b));
    let l20 = w20.iter().map(|b| b.low).fold(f64::INFINITY, |a, b| a.min(b));

    levels.push(h20);
    levels.push(l20);

    if bars.len() >= 50 {
        let w50 = &bars[bars.len().saturating_sub(50)..];
        let h50 = w50.iter().map(|b| b.high).fold(f64::NEG_INFINITY, |a, b| a.max(b));
        let l50 = w50.iter().map(|b| b.low).fold(f64::INFINITY, |a, b| a.min(b));
        if (h50 - h20).abs() > 0.01 { levels.push(h50); }
        if (l50 - l20).abs() > 0.01 { levels.push(l50); }
    }

    levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
    levels.dedup();
    levels
}

fn calculate_sma(data: &[f64], period: usize) -> Option<f64> {
    if data.len() < period {
        return None;
    }
    let window = &data[data.len() - period..];
    let sum: f64 = window.iter().sum();
    Some(sum / period as f64)
}

fn calculate_rsi(data: &[f64], period: usize) -> Option<f64> {
    if data.len() <= period {
        return None;
    }

    let mut gains = 0.0;
    let mut losses = 0.0;

    // Initial SMA of gains/losses
    for i in 1..=period {
        let change = data[i] - data[i - 1];
        if change > 0.0 {
            gains += change;
        } else {
            losses -= change;
        }
    }
    let mut avg_gain = gains / period as f64;
    let mut avg_loss = losses / period as f64;

    for i in (period + 1)..data.len() {
        let change = data[i] - data[i - 1];
        let (gain, loss) = if change > 0.0 { (change, 0.0) } else { (0.0, -change) };

        avg_gain = (avg_gain * (period as f64 - 1.0) + gain) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + loss) / period as f64;
    }

    if avg_loss == 0.0 {
        return Some(100.0);
    }

    let rs = avg_gain / avg_loss;
    Some(100.0 - (100.0 / (1.0 + rs)))
}

fn calculate_atr(bars: &[Bar], period: usize) -> Option<f64> {
    if bars.len() < period + 1 {
        return None;
    }

    let mut tr_sum = 0.0;
    // Initial TRs
    for i in 1..=period {
        let high = bars[i].high;
        let low = bars[i].low;
        let prev_close = bars[i - 1].close;

        let tr = (high - low).max((high - prev_close).abs()).max((low - prev_close).abs());
        tr_sum += tr;
    }

    let mut atr = tr_sum / period as f64;

    // Smoothing
    for i in (period + 1)..bars.len() {
        let high = bars[i].high;
        let low = bars[i].low;
        let prev_close = bars[i - 1].close;

        let tr = (high - low).max((high - prev_close).abs()).max((low - prev_close).abs());
        atr = (atr * (period as f64 - 1.0) + tr) / period as f64;
    }

    Some(atr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_bar(close: f64, time: i64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "crypto".to_string(),
            timeframe: "1m".to_string(),
            timestamp_unix_ms: time,
            open: close,
            high: close * 1.01,
            low: close * 0.99,
            close,
            volume: 1000.0,
        }
    }

    #[test]
    fn test_calculate_sma() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(calculate_sma(&data, 3), Some(4.0)); // (3+4+5)/3 = 4
        assert_eq!(calculate_sma(&data, 5), Some(3.0)); // (1+2+3+4+5)/5 = 3
        assert_eq!(calculate_sma(&data, 6), None);
    }

    #[test]
    fn test_calculate_rsi() {
        let data = vec![100.0; 20];
        assert_eq!(calculate_rsi(&data, 14), Some(100.0));
    }

    #[test]
    fn test_regime_detection_uptrend() {
        let mut bars = Vec::new();
        for i in 0..100 {
            let close = 100.0 + i as f64; // Steady uptrend
            bars.push(create_bar(close, i));
        }

        let analysis = analyze(&BarSeries {
            schema_version: "v0".to_string(),
            bars,
        });

        assert_eq!(analysis.regime, "Trending Up");
    }

    #[test]
    fn test_patterns_engulfing() {
        let mut bars = Vec::new();
        let base_bar = create_bar(100.0, 0);
        // Previous Red
        bars.push(Bar {
            open: 100.0, close: 95.0, high: 100.0, low: 95.0, ..base_bar.clone()
        });
        // Current Green Engulfing
        bars.push(Bar {
            open: 94.0, close: 101.0, high: 101.0, low: 94.0, ..base_bar.clone()
        });

        let patterns = detect_patterns(&bars);
        assert!(patterns.contains(&"Bullish Engulfing".to_string()));
    }
}
