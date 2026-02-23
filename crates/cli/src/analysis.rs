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

    let sentiment = match regime.as_str() {
        "Trending Up" => "Bullish",
        "Trending Down" => "Bearish",
        _ => "Neutral",
    }.to_string();

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

fn calculate_regime(bars: &[Bar]) -> String {
    if bars.len() < 10 {
        return "Unknown".to_string();
    }
    let window = &bars[bars.len().saturating_sub(10)..];
    let closes: Vec<f64> = window.iter().map(|b| b.close).collect();
    let avg: f64 = closes.iter().sum::<f64>() / closes.len() as f64;
    let last_close = closes.last().unwrap();

    if *last_close > avg * 1.01 {
        "Trending Up".to_string()
    } else if *last_close < avg * 0.99 {
        "Trending Down".to_string()
    } else {
        "Ranging".to_string()
    }
}

fn calculate_volatility(bars: &[Bar]) -> String {
    if bars.len() < 20 {
        return "Unknown".to_string();
    }
    let window = &bars[bars.len().saturating_sub(20)..];
    let ranges: Vec<f64> = window.iter().map(|b| b.high - b.low).collect();
    let avg_range: f64 = ranges.iter().sum::<f64>() / ranges.len() as f64;

    let last_close = window.last().unwrap().close;
    if avg_range / last_close > 0.02 {
        "High".to_string()
    } else if avg_range / last_close > 0.005 {
        "Medium".to_string()
    } else {
        "Low".to_string()
    }
}

fn detect_patterns(bars: &[Bar]) -> Vec<String> {
    let mut patterns = Vec::new();
    if let Some(bar) = bars.last() {
        let body = (bar.close - bar.open).abs();
        let upper_wick = bar.high - bar.close.max(bar.open);
        let lower_wick = bar.close.min(bar.open) - bar.low;
        let range = bar.high - bar.low;

        if lower_wick > body * 2.0 && upper_wick < body * 0.5 && range > 0.0 {
            patterns.push("Hammer".to_string());
        }
        if upper_wick > body * 2.0 && lower_wick < body * 0.5 && range > 0.0 {
            patterns.push("Shooting Star".to_string());
        }
    }
    patterns
}

fn identify_levels(bars: &[Bar]) -> Vec<f64> {
    if bars.len() < 20 {
        return vec![];
    }
    let window = &bars[bars.len().saturating_sub(20)..];
    let highest = window.iter().map(|b| b.high).fold(f64::NEG_INFINITY, |a, b| a.max(b));
    let lowest = window.iter().map(|b| b.low).fold(f64::INFINITY, |a, b| a.min(b));
    vec![highest, lowest]
}
