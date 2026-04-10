use anyhow::{Result, anyhow};
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Configuration for the Fear and Greed Index analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FearAndGreedConfig {
    /// The period length for calculating moving averages, momentum, and volatility (e.g., 20).
    pub period: usize,
}

impl Default for FearAndGreedConfig {
    fn default() -> Self {
        Self { period: 20 }
    }
}

/// The result of the Fear and Greed analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FearAndGreedReport {
    /// The overall Fear and Greed Index score (0.0 to 100.0).
    /// 0 = Extreme Fear, 100 = Extreme Greed.
    pub score: f64,
    /// Human-readable label (e.g., "Extreme Fear", "Fear", "Neutral", "Greed", "Extreme Greed").
    pub label: String,
    /// The momentum component score (0.0 to 100.0).
    pub momentum_score: f64,
    /// The volatility component score (0.0 to 100.0).
    pub volatility_score: f64,
    /// The volume component score (0.0 to 100.0).
    pub volume_score: f64,
}

/// Analyzes the market data to compute a Fear and Greed Index.
///
/// The index is synthesized from three components:
/// 1.  **Momentum (Price Action):** Current price vs. moving average.
/// 2.  **Volatility:** Current volatility vs. historical volatility.
/// 3.  **Volume:** Current buying/selling pressure vs. historical volume.
///
/// # Arguments
///
/// * `series` - The historical OHLCV (Open, High, Low, Close, Volume).
/// * `config` - Configuration for the analysis periods.
///
/// # Examples
///
/// ```
/// use contracts::{BarSeries, Bar};
/// use thales_cli::fear_and_greed::{analyze_fear_and_greed, FearAndGreedConfig};
///
/// let mut bars = vec![];
/// for i in 0..60 {
///     // Create a steady uptrend
///     let price = 100.0 + (i as f64) * 2.0;
///     bars.push(Bar {
///         symbol: "AAPL".to_string(),
///         market: "equities".to_string(),
///         timestamp_unix_ms: i * 1000,
///         open: price, high: price + 2.0, low: price - 2.0, close: price + 1.0,
///         volume: 1000.0 + (i as f64) * 10.0,
///         timeframe: "1d".to_string(),
///     });
/// }
///
/// let series = BarSeries { schema_version: "1.0".to_string(), bars };
/// let config = FearAndGreedConfig { period: 20 };
///
/// let report = analyze_fear_and_greed(&series, config).unwrap();
///
/// // In a steady uptrend with increasing volume, the score should lean towards Greed (> 50.0).
/// assert!(report.score > 50.0);
/// ```
pub fn analyze_fear_and_greed(
    series: &BarSeries,
    config: FearAndGreedConfig,
) -> Result<FearAndGreedReport> {
    if config.period == 0 {
        return Err(anyhow!("Period must be greater than 0."));
    }

    let min_bars_needed = config.period * 2 + 1; // +1 because we need prev_close for TR
    if series.bars.len() < min_bars_needed {
        return Err(anyhow!(
            "Not enough data to calculate Fear and Greed index. Need at least {} bars.",
            min_bars_needed
        ));
    }

    let n = series.bars.len();
    let p = config.period;

    // 1. Momentum Score (Price vs SMA)
    // Compare current price to SMA over the period.
    let recent_closes: Vec<f64> = series.bars[n - p..].iter().map(|b| b.close).collect();
    let sma: f64 = recent_closes.iter().sum::<f64>() / p as f64;
    let current_price = series.bars[n - 1].close;

    // Normalize momentum: if price > SMA, it's greedy. If price < SMA, it's fearful.
    // Let's say +10% over SMA is extreme greed (100), -10% under SMA is extreme fear (0).
    let deviation = if sma > 0.0 {
        (current_price - sma) / sma
    } else {
        0.0
    };
    let max_deviation = 0.10; // 10%
    let momentum_raw = ((deviation / max_deviation) * 50.0) + 50.0;

    // Handle potential NaN if math goes weird
    let momentum_score = if momentum_raw.is_nan() {
        50.0
    } else {
        momentum_raw.clamp(0.0, 100.0)
    };

    // 2. Volatility Score
    // Compare current True Range to Average True Range.
    // High volatility usually correlates with fear (panic selling).
    // Only calculate TR for the exact window we need: the last p * 2 bars
    let tr_start_idx = n - (p * 2);
    let mut trs = Vec::with_capacity(p * 2);

    for i in tr_start_idx..n {
        let prev_close = series.bars[i - 1].close;
        let high = series.bars[i].high;
        let low = series.bars[i].low;
        let tr = (high - low)
            .max((high - prev_close).abs())
            .max((low - prev_close).abs());
        trs.push(tr);
    }

    let current_atr: f64 = trs[trs.len() - p..].iter().sum::<f64>() / p as f64;
    let hist_atr: f64 = trs[trs.len() - (p * 2)..trs.len() - p].iter().sum::<f64>() / p as f64;

    // If current ATR is much higher than historical ATR, it's fear.
    // Let's say 2x historical ATR is extreme fear (0 score), 0.5x is extreme greed (100 score).
    let vol_ratio = if hist_atr > 0.0 {
        current_atr / hist_atr
    } else {
        1.0 // If historical ATR is 0, assume neutral volatility change
    };

    // Map: ratio 0.5 -> 100, ratio 1.0 -> 50, ratio 2.0 -> 0.
    // Let's use a simple linear mapping inverted.
    // score = 100 - (ratio - 0.5) * (100 / 1.5)
    let vol_raw = 100.0 - (vol_ratio - 0.5) * (100.0 / 1.5);

    let volatility_score = if vol_raw.is_nan() {
        50.0
    } else {
        vol_raw.clamp(0.0, 100.0)
    };

    // 3. Volume Score
    // Buying vs Selling pressure.
    // Simple approach: Volume on up days vs volume on down days over the period.
    let mut up_volume = 0.0;
    let mut down_volume = 0.0;

    for i in n - p..n {
        let bar = &series.bars[i];
        let prev_close = series.bars[i - 1].close;
        if bar.close > prev_close {
            up_volume += bar.volume;
        } else if bar.close < prev_close {
            down_volume += bar.volume;
        }
    }

    let total_volume = up_volume + down_volume;
    let volume_score = if total_volume > 0.0 {
        (up_volume / total_volume) * 100.0
    } else {
        50.0
    };

    // Combine Scores (Weighted Average)
    // Momentum (40%), Volatility (30%), Volume (30%)
    let final_score = (momentum_score * 0.4) + (volatility_score * 0.3) + (volume_score * 0.3);

    // Determine Label
    let label = match final_score {
        s if s <= 20.0 => "Extreme Fear",
        s if s <= 45.0 => "Fear",
        s if s <= 55.0 => "Neutral",
        s if s <= 80.0 => "Greed",
        _ => "Extreme Greed",
    }
    .to_string();

    Ok(FearAndGreedReport {
        score: final_score,
        label,
        momentum_score,
        volatility_score,
        volume_score,
    })
}

/// Prints a visual ASCII representation of the Fear and Greed Index.
pub fn print_ascii_fear_and_greed(report: &FearAndGreedReport) {
    println!("\n==============================================");
    println!("          FEAR AND GREED INDEX");
    println!("==============================================\n");

    println!("Current Score: {:.1} / 100", report.score);
    println!("Status:        {}\n", report.label);

    // Draw meter
    let meter_len = 40;
    let fill = ((report.score / 100.0) * meter_len as f64).round() as usize;

    print!("Meter: [");
    for i in 0..meter_len {
        if i < fill {
            if i < meter_len / 4 {
                print!("R"); // Red / Fear
            } else if i < meter_len * 3 / 4 {
                print!("-"); // Neutral
            } else {
                print!("G"); // Green / Greed
            }
        } else {
            print!(" ");
        }
    }
    println!("]\n");

    println!("--- Components ---");
    println!("Momentum Score:   {:.1}", report.momentum_score);
    println!("Volatility Score: {:.1}", report.volatility_score);
    println!("Volume Score:     {:.1}", report.volume_score);
    println!("\n==============================================\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_test_series(trend: &str) -> BarSeries {
        let mut bars = Vec::new();
        let mut price = 100.0;

        // Generate 50 bars
        for i in 0..50 {
            let change = match trend {
                "up" => 1.0,    // Consistent up trend
                "down" => -1.0, // Consistent down trend
                _ => 0.0,       // Flat
            };

            price += change;

            bars.push(Bar {
                symbol: "TEST".to_string(),
                market: "crypto".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 1000000 + (i * 86400000),
                open: price - change,
                high: price + 0.5,
                low: price - 1.5,
                close: price,
                volume: 1000.0,
            });
        }

        BarSeries {
            schema_version: "v0".to_string(),
            bars,
        }
    }

    #[test]
    fn test_fear_and_greed_uptrend() {
        let series = create_test_series("up");
        let config = FearAndGreedConfig { period: 20 };
        let report = analyze_fear_and_greed(&series, config).unwrap();

        // In a consistent uptrend, the score should be heavily leaning towards Greed.
        assert!(report.score > 55.0, "Score should indicate Greed");
        assert!(report.momentum_score > 50.0);
        assert!(report.volume_score > 50.0);
    }

    #[test]
    fn test_fear_and_greed_downtrend() {
        let series = create_test_series("down");
        let config = FearAndGreedConfig { period: 20 };
        let report = analyze_fear_and_greed(&series, config).unwrap();

        // In a consistent downtrend, the score should be heavily leaning towards Fear.
        assert!(report.score < 45.0, "Score should indicate Fear");
        assert!(report.momentum_score < 50.0);
        assert!(report.volume_score < 50.0);
    }

    #[test]
    fn test_fear_and_greed_not_enough_data() {
        let mut series = create_test_series("up");
        series.bars.truncate(10); // Leave only 10 bars

        let config = FearAndGreedConfig { period: 20 }; // Needs at least 40
        let result = analyze_fear_and_greed(&series, config);

        assert!(result.is_err());
    }
}
