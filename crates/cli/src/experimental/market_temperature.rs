use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureConfig {
    pub window_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureReport {
    pub temperature: f64,
}

pub fn analyze_temperature(
    series: &BarSeries,
    config: TemperatureConfig,
) -> Result<TemperatureReport> {
    if series.bars.len() < config.window_size || config.window_size == 0 {
        return Ok(TemperatureReport { temperature: 0.0 });
    }

    let start_idx = series.bars.len() - config.window_size;
    let window = &series.bars[start_idx..];

    let mut total_volume = 0.0;
    let mut weighted_roc = 0.0;

    for i in 1..window.len() {
        let prev_close = window[i - 1].close;
        let current_close = window[i].close;
        let volume = window[i].volume;

        if prev_close > 0.0 {
            let roc = (current_close - prev_close) / prev_close;
            weighted_roc += roc.abs() * volume;
        }
        total_volume += volume;
    }

    let temperature = if total_volume > 0.0 {
        (weighted_roc / total_volume) * 10000.0 // Scaled for readability
    } else {
        0.0
    };

    Ok(TemperatureReport { temperature })
}

pub fn print_ascii_temperature(report: &TemperatureReport) {
    println!("Market Temperature Report");
    println!("-------------------------");
    println!("Temperature: {:.2}°", report.temperature);
    let bar_len = (report.temperature / 10.0).min(50.0) as usize;
    let bar: String = "🔥".repeat(bar_len.max(1));
    println!("Heat Index:  [{}]", bar);
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_analyze_temperature() {
        let series = BarSeries {
            schema_version: "v1".to_string(),
            bars: vec![
                Bar {
                    symbol: "BTC".to_string(),
                    market: "crypto".to_string(),
                    timeframe: "1d".to_string(),
                    timestamp_unix_ms: 0,
                    open: 100.0,
                    high: 110.0,
                    low: 90.0,
                    close: 105.0,
                    volume: 1000.0,
                },
                Bar {
                    symbol: "BTC".to_string(),
                    market: "crypto".to_string(),
                    timeframe: "1d".to_string(),
                    timestamp_unix_ms: 1,
                    open: 105.0,
                    high: 120.0,
                    low: 100.0,
                    close: 115.0,
                    volume: 2000.0,
                },
            ],
        };
        let config = TemperatureConfig { window_size: 2 };
        let result = analyze_temperature(&series, config);
        assert!(result.is_ok());
    }
}
