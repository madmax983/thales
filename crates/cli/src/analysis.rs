use contracts::{Bar, BarSeries, MarketAnalysis};
use polars::prelude::*;
use strategies::indicators::{atr, bollinger_bands, donchian_channels, macd, rsi, sma};

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
            atr: None,
            research_summary: None,
            news_summary: None,
            recommendation: None,
            confidence: 0.0,
            timestamp_unix_ms: timestamp,
        };
    }

    // Convert to DataFrame for indicators
    let df = match bars_to_df(bars) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error converting bars to DataFrame: {}", e);
            return MarketAnalysis {
                symbol,
                market,
                regime: "Error".to_string(),
                sentiment: "Error".to_string(),
                patterns: vec![],
                key_levels: vec![],
                volatility: "Error".to_string(),
                atr: None,
                research_summary: None,
                news_summary: None,
                recommendation: None,
                confidence: 0.0,
                timestamp_unix_ms: timestamp,
            };
        }
    };

    let regime = calculate_regime(&df);
    let (volatility, atr_val) = calculate_volatility(&df, bars);
    let sentiment = calculate_sentiment(&df, &regime);
    let patterns = detect_patterns(&df, bars);
    let key_levels = identify_levels(&df);

    let recommendation = calculate_recommendation(&regime, &volatility, &sentiment);
    let confidence = calculate_confidence(&regime, &volatility, &sentiment, &patterns);

    MarketAnalysis {
        symbol,
        market,
        regime,
        sentiment,
        patterns,
        key_levels,
        volatility,
        atr: atr_val,
        research_summary: None,
        news_summary: None,
        recommendation: Some(recommendation),
        confidence,
        timestamp_unix_ms: timestamp,
    }
}

fn calculate_confidence(
    regime: &str,
    volatility: &str,
    sentiment: &str,
    patterns: &[String],
) -> f64 {
    let mut score: f64 = 0.5;

    // Regime Alignment
    if regime.contains("Trending") {
        score += 0.2;
    }

    // Sentiment Alignment
    if (regime.contains("Trending Up") && sentiment.contains("Bullish"))
        || (regime.contains("Trending Down") && sentiment.contains("Bearish"))
    {
        score += 0.1;
    }

    // Volatility Penalty
    if volatility == "Extreme" {
        score -= 0.2;
    } else if volatility == "High" {
        score -= 0.1;
    }

    // Pattern Bonus
    for pattern in patterns {
        if (regime.contains("Trending Up")
            && (pattern.contains("Bullish") || pattern == "Hammer" || pattern == "Breakout"))
            || (regime.contains("Trending Down")
                && (pattern.contains("Bearish")
                    || pattern == "Shooting Star"
                    || pattern == "Breakout"))
        {
            score += 0.1;
            break; // Cap bonus
        }
    }

    score.clamp(0.0, 1.0)
}

fn calculate_recommendation(regime: &str, volatility: &str, _sentiment: &str) -> String {
    if volatility == "High" || volatility == "Extreme" {
        return "Reduce Risk / Wait for Clarity".to_string();
    }
    match regime {
        "Trending Up" => "Trend Following (Long)".to_string(),
        "Trending Down" => "Trend Following (Short)".to_string(),
        "Ranging" => "Mean Reversion".to_string(),
        r if r.contains("Trending Up") => "Trend Following (Long)".to_string(),
        r if r.contains("Trending Down") => "Trend Following (Short)".to_string(),
        _ => "Neutral / Wait".to_string(),
    }
}

fn calculate_regime(df: &DataFrame) -> String {
    // SMA 50 vs SMA 200
    let sma50 = sma::calculate(df, 50).ok();
    let sma200 = sma::calculate(df, 200).ok();

    if let (Some(s50), Some(s200)) = (sma50, sma200) {
        // Get last valid values
        let s50_last = s50.f64().ok().and_then(|s| s.last());
        let s200_last = s200.f64().ok().and_then(|s| s.last());

        if let (Some(v50), Some(v200)) = (s50_last, s200_last) {
            if v50 > v200 * 1.01 {
                return "Trending Up".to_string();
            } else if v50 < v200 * 0.99 {
                return "Trending Down".to_string();
            }
        }
    }

    // Fallback: Price vs SMA 20 (Short term trend)
    let sma20 = sma::calculate(df, 20).ok();
    let close = df.column("close").ok().and_then(|c| c.f64().ok());

    if let (Some(s20), Some(c)) = (sma20, close) {
        let s20_last = s20.f64().ok().and_then(|s| s.last());
        let close_last = c.last();

        if let (Some(v20), Some(vc)) = (s20_last, close_last) {
            if vc > v20 * 1.01 {
                return "Trending Up (Short Term)".to_string();
            } else if vc < v20 * 0.99 {
                return "Trending Down (Short Term)".to_string();
            }
        }
    }

    "Ranging".to_string()
}

fn calculate_volatility(df: &DataFrame, bars: &[Bar]) -> (String, Option<f64>) {
    let atr_series = atr::calculate(df, 14).ok();
    let last_close = bars.last().map(|b| b.close).unwrap_or(1.0);

    let mut atr_val = None;

    if let Some(s) = atr_series {
        if let Some(last) = s.f64().ok().and_then(|v| v.last()) {
            atr_val = Some(last);
            let ratio = last / last_close;

            if ratio > 0.05 {
                return ("Extreme".to_string(), atr_val);
            } else if ratio > 0.02 {
                return ("High".to_string(), atr_val);
            } else if ratio > 0.01 {
                return ("Medium".to_string(), atr_val);
            } else {
                return ("Low".to_string(), atr_val);
            }
        }
    }

    ("Unknown".to_string(), atr_val)
}

fn calculate_sentiment(df: &DataFrame, regime: &str) -> String {
    let rsi_series = rsi::calculate(df, 14).ok();
    let macd_res = macd::calculate(df, 12, 26, 9).ok();

    let mut sentiment_score = 0; // -2 to +2
    let mut last_rsi_val = None;

    if let Some(s) = rsi_series {
        if let Some(val) = s.f64().ok().and_then(|v| v.last()) {
            last_rsi_val = Some(val);
            if val > 70.0 {
                sentiment_score += 1;
            }
            // Bullish (Overbought in strong trend)
            else if val < 30.0 {
                sentiment_score -= 1;
            }
            // Bearish
            else if val > 55.0 {
                sentiment_score += 1;
            } else if val < 45.0 {
                sentiment_score -= 1;
            }
        }
    }

    if let Some((macd_line, signal_line, _hist)) = macd_res {
        let m = macd_line.f64().ok().and_then(|v| v.last());
        let s = signal_line.f64().ok().and_then(|v| v.last());

        if let (Some(mv), Some(sv)) = (m, s) {
            if mv > sv {
                sentiment_score += 1;
            } else {
                sentiment_score -= 1;
            }
        }
    }

    let mut sentiment_str = if sentiment_score >= 2 {
        "Bullish (Strong)".to_string()
    } else if sentiment_score == 1 {
        "Bullish".to_string()
    } else if sentiment_score == -1 {
        "Bearish".to_string()
    } else if sentiment_score <= -2 {
        "Bearish (Strong)".to_string()
    } else {
        match regime {
            "Trending Up" => "Bullish".to_string(),
            "Trending Down" => "Bearish".to_string(),
            _ => "Neutral".to_string(),
        }
    };

    if let Some(val) = last_rsi_val {
        if val > 70.0 {
            sentiment_str.push_str(" (Overbought)");
        } else if val < 30.0 {
            sentiment_str.push_str(" (Oversold)");
        }
    }

    sentiment_str
}

fn detect_patterns(df: &DataFrame, bars: &[Bar]) -> Vec<String> {
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
        if curr_body <= curr_range * 0.05 {
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

    // Structural Patterns
    // 1. Breakout (Donchian)
    if let Ok((upper, lower, _mid)) = donchian_channels::calculate(df, 20) {
        let u = upper.f64().ok().and_then(|s| s.get(bars.len() - 2)); // Previous high
        let l = lower.f64().ok().and_then(|s| s.get(bars.len() - 2)); // Previous low

        // Note: Donchian implementation usually shifts, so index might be tricky.
        // Assuming strategies implementation: if i=20, value is max(0..19).
        // If Price > Upper[prev], it's a breakout.

        if let Some(uv) = u {
            if curr.close > uv {
                patterns.push("Breakout (Upside)".to_string());
            }
        }
        if let Some(lv) = l {
            if curr.close < lv {
                patterns.push("Breakout (Downside)".to_string());
            }
        }
    }

    // 2. Squeeze (Bollinger Band Width)
    if let Ok((upper, lower, _mid)) = bollinger_bands::calculate(df, 20, 2.0) {
        let u = upper.f64().ok().and_then(|s| s.last());
        let l = lower.f64().ok().and_then(|s| s.last());
        let m = _mid.f64().ok().and_then(|s| s.last());

        if let (Some(uv), Some(lv), Some(mv)) = (u, l, m) {
            let width = (uv - lv) / mv;
            // Heuristic: Width < 0.02 (2%) is tight for many assets, but asset dependent.
            // Better to compare to historical average width, but simplicity first.
            if width < 0.015 {
                patterns.push("Consolidation (Squeeze)".to_string());
            }
        }
    }

    patterns
}

fn identify_levels(df: &DataFrame) -> Vec<f64> {
    let mut levels = Vec::new();

    // Donchian Channels (20, 50)
    if let Ok((upper, lower, _)) = donchian_channels::calculate(df, 20) {
        if let Some(v) = upper.f64().ok().and_then(|s| s.last()) {
            levels.push(v);
        }
        if let Some(v) = lower.f64().ok().and_then(|s| s.last()) {
            levels.push(v);
        }
    }
    if let Ok((upper, lower, _)) = donchian_channels::calculate(df, 50) {
        if let Some(v) = upper.f64().ok().and_then(|s| s.last()) {
            levels.push(v);
        }
        if let Some(v) = lower.f64().ok().and_then(|s| s.last()) {
            levels.push(v);
        }
    }

    levels.sort_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    levels.dedup();
    levels
}

fn bars_to_df(bars: &[Bar]) -> anyhow::Result<DataFrame> {
    let opens: Vec<f64> = bars.iter().map(|b| b.open).collect();
    let highs: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let lows: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let volumes: Vec<f64> = bars.iter().map(|b| b.volume).collect();
    let times: Vec<i64> = bars.iter().map(|b| b.timestamp_unix_ms).collect();

    let df = df!(
        "open" => opens,
        "high" => highs,
        "low" => lows,
        "close" => closes,
        "volume" => volumes,
        "timestamp_unix_ms" => times
    )?;
    Ok(df)
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
    fn test_analyze_uptrend() {
        let mut bars = Vec::new();
        // Generate 200 bars of uptrend
        for i in 0..200 {
            let close = 100.0 + (i as f64);
            bars.push(create_bar(close, i));
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let analysis = analyze(&series);
        assert!(analysis.regime.contains("Trending Up"));
        assert!(analysis.confidence > 0.5);
    }

    #[test]
    fn test_analyze_patterns() {
        let mut bars = Vec::new();
        let base_bar = create_bar(100.0, 0);
        // Previous Red
        bars.push(Bar {
            open: 100.0,
            close: 95.0,
            high: 100.0,
            low: 95.0,
            ..base_bar.clone()
        });
        // Current Green Engulfing
        bars.push(Bar {
            open: 94.0,
            close: 101.0,
            high: 101.0,
            low: 94.0,
            ..base_bar.clone()
        });

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let analysis = analyze(&series);
        assert!(analysis.patterns.contains(&"Bullish Engulfing".to_string()));
    }

    #[test]
    fn test_volatility_classification() {
        let mut bars = Vec::new();
        // Extremely volatile: 10% jumps
        for i in 0..50 {
            let close = if i % 2 == 0 { 100.0 } else { 110.0 };
            bars.push(Bar {
                open: close,
                close,
                high: close * 1.05,
                low: close * 0.95,
                ..create_bar(close, i)
            });
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let analysis = analyze(&series);

        // ATR will be high relative to price (avg price ~105, TR ~15)
        // Ratio ~ 15/105 ~ 0.14 > 0.05 -> Extreme
        assert_eq!(analysis.volatility, "Extreme");
    }

    #[test]
    fn test_sentiment_overbought_oversold() {
        let mut bars = Vec::new();
        // Generate Overbought (RSI > 70)
        // Steep uptrend usually causes high RSI
        for i in 0..50 {
            let close = 100.0 + (i as f64) * 2.0; // Steep climb
            bars.push(create_bar(close, i));
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };
        let analysis = analyze(&series);
        assert!(
            analysis.sentiment.contains("(Overbought)"),
            "Sentiment '{}' should contain (Overbought)",
            analysis.sentiment
        );

        // Generate Oversold (RSI < 30)
        let mut bars_down = Vec::new();
        for i in 0..50 {
            let close = 200.0 - (i as f64) * 2.0; // Steep drop
            bars_down.push(create_bar(close, i));
        }
        let series_down = BarSeries {
            schema_version: "v0".to_string(),
            bars: bars_down,
        };
        let analysis_down = analyze(&series_down);
        assert!(
            analysis_down.sentiment.contains("(Oversold)"),
            "Sentiment '{}' should contain (Oversold)",
            analysis_down.sentiment
        );
    }
}
