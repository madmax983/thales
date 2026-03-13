use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Configuration for the Market Phases analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPhasesConfig {
    /// The short-term moving average window.
    pub short_window: usize,
    /// The long-term moving average window.
    pub long_window: usize,
}

/// The result of a Market Phases analysis run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPhasesReport {
    /// The current identified market phase.
    pub current_phase: String,
    /// The moving average values.
    pub short_ma: f64,
    pub long_ma: f64,
}

/// Analyzes a [`BarSeries`] to classify the current market phase.
///
/// Returns an error if:
/// - `BarSeries` is empty.
/// - The dataset has fewer points than `config.long_window`.
pub fn analyze_market_phases(
    series: &BarSeries,
    config: MarketPhasesConfig,
) -> Result<MarketPhasesReport> {
    if series.bars.is_empty() {
        return Err(anyhow::anyhow!("Bar series cannot be empty"));
    }

    if config.short_window == 0 || config.long_window == 0 {
        return Err(anyhow::anyhow!("Window sizes must be greater than 0"));
    }

    if config.short_window >= config.long_window {
        return Err(anyhow::anyhow!(
            "short_window must be strictly less than long_window"
        ));
    }

    if series.bars.len() < config.long_window {
        return Err(anyhow::anyhow!(
            "Not enough data points (found {}, need {})",
            series.bars.len(),
            config.long_window
        ));
    }

    let closes: Vec<f64> = series.bars.iter().map(|b| b.close).collect();

    let short_ma = closes[closes.len() - config.short_window..]
        .iter()
        .sum::<f64>()
        / config.short_window as f64;
    let long_ma = closes[closes.len() - config.long_window..]
        .iter()
        .sum::<f64>()
        / config.long_window as f64;

    let current_close = closes.last().unwrap();

    // Very basic phase detection
    // Accumulation: Price ~ MA, low volatility (simplified here)
    // Markup: Price > Short MA > Long MA
    // Distribution: Price ~ MA, high volatility after Markup (simplified here)
    // Markdown: Price < Short MA < Long MA
    let current_phase = if current_close > &short_ma && short_ma > long_ma {
        "Markup".to_string()
    } else if current_close < &short_ma && short_ma < long_ma {
        "Markdown".to_string()
    } else if current_close > &long_ma {
        "Distribution".to_string()
    } else {
        "Accumulation".to_string()
    };

    Ok(MarketPhasesReport {
        current_phase,
        short_ma,
        long_ma,
    })
}

#[cfg(feature = "nova")]
pub fn print_ascii_market_phases(report: &MarketPhasesReport) {
    println!("\nMarket Phases Analysis");
    println!("--------------------------------------------------");
    println!("Current Phase:       {}", report.current_phase);
    println!("Short MA:            {:.2}", report.short_ma);
    println!("Long MA:             {:.2}", report.long_ma);
    println!("--------------------------------------------------\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_bar(timestamp: i64, close: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
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
    fn test_market_phases_markup() {
        let mut bars = Vec::new();
        for i in 0..200 {
            bars.push(create_bar(i as i64 * 1000, 10.0 + (i as f64 * 0.1))); // Upward trend
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = MarketPhasesConfig {
            short_window: 50,
            long_window: 200,
        };
        let report = analyze_market_phases(&series, config).expect("Should succeed");

        assert_eq!(report.current_phase, "Markup");
    }
}
