use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairsConfig {
    pub zscore_window: usize,
    pub divergence_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairsReport {
    pub symbol_a: String,
    pub symbol_b: String,
    pub correlation: f64,
    pub current_zscore: f64,
    pub is_diverged: bool,
    pub recent_spread: Vec<f64>,
}

pub fn analyze_pairs(
    series_a: &BarSeries,
    series_b: &BarSeries,
    config: PairsConfig,
) -> Result<PairsReport> {
    if series_a.bars.is_empty() || series_b.bars.is_empty() {
        return Err(anyhow::anyhow!("Bar series cannot be empty"));
    }

    if config.zscore_window == 0 {
        return Err(anyhow::anyhow!("zscore_window must be > 0"));
    }

    // Align series by timestamp using a simple two-pointer approach
    let mut i = 0;
    let mut j = 0;
    let mut prices_a = Vec::new();
    let mut prices_b = Vec::new();

    while i < series_a.bars.len() && j < series_b.bars.len() {
        let bar_a = &series_a.bars[i];
        let bar_b = &series_b.bars[j];

        if bar_a.timestamp_unix_ms == bar_b.timestamp_unix_ms {
            prices_a.push(bar_a.close);
            prices_b.push(bar_b.close);
            i += 1;
            j += 1;
        } else if bar_a.timestamp_unix_ms < bar_b.timestamp_unix_ms {
            i += 1;
        } else {
            j += 1;
        }
    }

    if prices_a.len() < config.zscore_window {
        return Err(anyhow::anyhow!(
            "Not enough overlapping data points (found {}, need {})",
            prices_a.len(),
            config.zscore_window
        ));
    }

    // Calculate correlation over the available aligned window
    let correlation = calculate_correlation(&prices_a, &prices_b);

    // Calculate spread = Price A - Price B (simplified) or log(Price A) - log(Price B)
    // We use a simple ratio spread here for stability
    let spread: Vec<f64> = prices_a
        .iter()
        .zip(prices_b.iter())
        .map(|(a, b)| a / b)
        .collect();

    // Calculate rolling Z-Score for the recent window
    let window_start = spread.len() - config.zscore_window;
    let recent_spread = &spread[window_start..];

    let mean = recent_spread.iter().sum::<f64>() / recent_spread.len() as f64;
    let variance = recent_spread
        .iter()
        .map(|s| (s - mean).powi(2))
        .sum::<f64>()
        / recent_spread.len() as f64;
    let std_dev = variance.sqrt();

    let current_zscore = if std_dev > 0.0 {
        (recent_spread.last().unwrap() - mean) / std_dev
    } else {
        0.0
    };

    let is_diverged = current_zscore.abs() > config.divergence_threshold;

    Ok(PairsReport {
        symbol_a: series_a.bars[0].symbol.clone(),
        symbol_b: series_b.bars[0].symbol.clone(),
        correlation,
        current_zscore,
        is_diverged,
        recent_spread: recent_spread.to_vec(),
    })
}

fn calculate_correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let sum_x: f64 = x.iter().sum();
    let sum_y: f64 = y.iter().sum();
    let sum_xy: f64 = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum();
    let sum_x_sq: f64 = x.iter().map(|a| a * a).sum();
    let sum_y_sq: f64 = y.iter().map(|a| a * a).sum();

    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_x_sq - sum_x.powi(2)) * (n * sum_y_sq - sum_y.powi(2))).sqrt();

    if denominator == 0.0 {
        return 0.0;
    }

    numerator / denominator
}

#[cfg(feature = "nova")]
pub fn print_ascii_pairs(report: &PairsReport) {
    println!(
        "\nPairs Trading Analysis: {} vs {}",
        report.symbol_a, report.symbol_b
    );
    println!("--------------------------------------------------");
    println!("Correlation:         {:.4}", report.correlation);
    println!("Current Spread Z:    {:.2}", report.current_zscore);

    let divergence_color = if report.is_diverged {
        "\x1b[1;31m" // Red for action/divergence
    } else {
        "\x1b[1;32m" // Green for normal
    };
    println!(
        "State:               {}{} \x1b[0m",
        divergence_color,
        if report.is_diverged {
            "DIVERGED"
        } else {
            "NORMAL"
        }
    );
    println!("--------------------------------------------------\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_bar(symbol: &str, timestamp: i64, close: f64) -> Bar {
        Bar {
            symbol: symbol.to_string(),
            market: "equities".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: timestamp,
            open: close,
            high: close,
            low: close,
            close,
            volume: 100.0,
        }
    }

    #[test]
    fn test_pairs_correlation() {
        let bars_a = vec![
            create_bar("A", 1000, 10.0),
            create_bar("A", 2000, 12.0),
            create_bar("A", 3000, 14.0),
            create_bar("A", 4000, 16.0),
        ];

        let bars_b = vec![
            create_bar("B", 1000, 20.0),
            create_bar("B", 2000, 24.0),
            create_bar("B", 3000, 28.0),
            create_bar("B", 4000, 32.0),
        ];

        let series_a = BarSeries {
            schema_version: "v0".to_string(),
            bars: bars_a,
        };
        let series_b = BarSeries {
            schema_version: "v0".to_string(),
            bars: bars_b,
        };

        let config = PairsConfig {
            zscore_window: 4,
            divergence_threshold: 2.0,
        };
        let report = analyze_pairs(&series_a, &series_b, config).expect("Should succeed");

        assert_eq!(report.symbol_a, "A");
        assert_eq!(report.symbol_b, "B");
        assert!(
            (report.correlation - 1.0).abs() < 1e-6,
            "Correlation should be 1.0"
        );
    }

    #[test]
    fn test_pairs_divergence() {
        let bars_a = vec![
            create_bar("A", 1000, 10.0),
            create_bar("A", 2000, 10.0),
            create_bar("A", 3000, 10.0),
            create_bar("A", 4000, 20.0), // Spikes up
        ];

        let bars_b = vec![
            create_bar("B", 1000, 10.0),
            create_bar("B", 2000, 10.0),
            create_bar("B", 3000, 10.0),
            create_bar("B", 4000, 10.0), // Stays flat
        ];

        let series_a = BarSeries {
            schema_version: "v0".to_string(),
            bars: bars_a,
        };
        let series_b = BarSeries {
            schema_version: "v0".to_string(),
            bars: bars_b,
        };

        let config = PairsConfig {
            zscore_window: 4,
            divergence_threshold: 1.0,
        };
        let report = analyze_pairs(&series_a, &series_b, config).expect("Should succeed");

        assert!(report.current_zscore > 1.0, "Z-score should spike positive");
        assert!(report.is_diverged, "Pairs should be marked as diverged");
    }
}
