use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SeasonalityPeriod {
    DayOfWeek,
    MonthOfYear,
    HourOfDay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalityConfig {
    pub period: SeasonalityPeriod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalityReport {
    pub symbol: String,
    pub period: SeasonalityPeriod,
    pub bins: Vec<SeasonalityBin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalityBin {
    pub label: String,
    pub average_return: f64,
    pub win_rate: f64,
    pub count: usize,
}

use chrono::{DateTime, Datelike, Timelike};
use std::collections::HashMap;

pub fn analyze_seasonality(
    series: &BarSeries,
    config: SeasonalityConfig,
) -> Result<SeasonalityReport> {
    if series.bars.len() < 2 {
        return Err(anyhow::anyhow!(
            "At least two bars are required for seasonality analysis"
        ));
    }

    let symbol = series.bars[0].symbol.clone();

    // Group returns by the chosen period
    let mut period_returns: HashMap<u32, Vec<f64>> = HashMap::new();

    for i in 1..series.bars.len() {
        let prev = &series.bars[i - 1];
        let curr = &series.bars[i];

        if prev.close == 0.0 {
            continue;
        }

        let ret = (curr.close - prev.close) / prev.close;

        // Convert timestamp to DateTime
        let dt = DateTime::from_timestamp(curr.timestamp_unix_ms / 1000, 0).unwrap_or_default();

        let key = match config.period {
            SeasonalityPeriod::DayOfWeek => dt.weekday().number_from_monday(), // 1=Mon, 7=Sun
            SeasonalityPeriod::MonthOfYear => dt.month(),                      // 1=Jan, 12=Dec
            SeasonalityPeriod::HourOfDay => dt.hour(),                         // 0..23
        };

        period_returns.entry(key).or_default().push(ret);
    }

    let mut bins = Vec::new();

    for (key, returns) in period_returns {
        let count = returns.len();
        if count == 0 {
            continue;
        }

        let total_return: f64 = returns.iter().sum();
        let average_return = total_return / count as f64;

        let wins = returns.iter().filter(|&&r| r > 0.0).count();
        let win_rate = wins as f64 / count as f64;

        let label = match config.period {
            SeasonalityPeriod::DayOfWeek => match key {
                1 => "Monday",
                2 => "Tuesday",
                3 => "Wednesday",
                4 => "Thursday",
                5 => "Friday",
                6 => "Saturday",
                7 => "Sunday",
                _ => "Unknown",
            }
            .to_string(),
            SeasonalityPeriod::MonthOfYear => match key {
                1 => "Jan",
                2 => "Feb",
                3 => "Mar",
                4 => "Apr",
                5 => "May",
                6 => "Jun",
                7 => "Jul",
                8 => "Aug",
                9 => "Sep",
                10 => "Oct",
                11 => "Nov",
                12 => "Dec",
                _ => "Unknown",
            }
            .to_string(),
            SeasonalityPeriod::HourOfDay => format!("{:02}:00", key),
        };

        bins.push(SeasonalityBin {
            label,
            average_return,
            win_rate,
            count,
        });
    }

    // Fill in missing bins with 0 data so output is consistent (e.g. 7 days, 12 months)
    // This also naturally sorts the output by using an ordered list of labels.
    let full_labels: Vec<String> = match config.period {
        SeasonalityPeriod::DayOfWeek => vec![
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect(),
        SeasonalityPeriod::MonthOfYear => vec![
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect(),
        SeasonalityPeriod::HourOfDay => (0..24).map(|h| format!("{:02}:00", h)).collect(),
    };

    let mut full_bins = Vec::new();
    for label in full_labels {
        if let Some(existing) = bins.iter().find(|b| b.label == label) {
            full_bins.push(existing.clone());
        } else {
            full_bins.push(SeasonalityBin {
                label,
                average_return: 0.0,
                win_rate: 0.0,
                count: 0,
            });
        }
    }

    Ok(SeasonalityReport {
        symbol,
        period: config.period,
        bins: full_bins,
    })
}

#[cfg(feature = "nova")]
pub fn print_ascii_seasonality(report: &SeasonalityReport) {
    println!(
        "\nSeasonality Analysis: {} ({:?})",
        report.symbol, report.period
    );
    println!("--------------------------------------------------");
    println!(
        "{:<10} | {:>10} | {:>8} | {:>5}",
        "Period", "Avg Return", "Win Rate", "Count"
    );
    println!("--------------------------------------------------");

    for bin in &report.bins {
        let return_color = if bin.average_return > 0.0 {
            "\x1b[1;32m" // Green
        } else if bin.average_return < 0.0 {
            "\x1b[1;31m" // Red
        } else {
            "\x1b[1;30m" // Dark Gray
        };

        println!(
            "{:<10} | {}{:9.4}%\x1b[0m | {:7.2}% | {:5}",
            bin.label,
            return_color,
            bin.average_return * 100.0,
            bin.win_rate * 100.0,
            bin.count
        );
    }
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
    fn test_seasonality_day_of_week() {
        let bars = vec![
            create_bar(1672617600000, 100.0), // Monday
            create_bar(1672704000000, 105.0), // Tuesday (+5%)
            create_bar(1672790400000, 100.0), // Wednesday (-4.7%)
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = SeasonalityConfig {
            period: SeasonalityPeriod::DayOfWeek,
        };

        let report = analyze_seasonality(&series, config).unwrap();
        assert_eq!(report.bins.len(), 7); // Should return 7 days
    }
}
