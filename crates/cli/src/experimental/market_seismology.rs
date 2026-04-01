#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Configuration for market seismology analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeismologyConfig {
    /// Number of bars to consider for the baseline "ambient" movement.
    pub window_size: usize,
    /// Multiplier above the ambient noise to be considered an earthquake (tremor).
    pub tremor_threshold: f64,
}

/// A report detailing the seismic activity of the market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeismicReport {
    /// The ambient noise level (average true range or similar over window).
    pub ambient_noise: f64,
    /// The current magnitude of the "quake" (latest bar movement vs ambient).
    pub current_magnitude: f64,
    /// Has a tremor been detected based on the threshold?
    pub is_tremor: bool,
    /// Richter scale equivalent (logarithmic score of movement).
    pub richter_scale: f64,
}

/// Analyzes the market series for seismic activity.
pub fn analyze_seismology(
    series: &BarSeries,
    config: SeismologyConfig,
) -> anyhow::Result<SeismicReport> {
    if series.bars.len() < config.window_size || config.window_size < 1 {
        return Ok(SeismicReport {
            ambient_noise: 0.0,
            current_magnitude: 0.0,
            is_tremor: false,
            richter_scale: 0.0,
        });
    }

    let n = series.bars.len();
    let mut total_range = 0.0;

    // Calculate ambient noise as the average range over the window
    for i in (n - config.window_size)..(n - 1) {
        let bar = &series.bars[i];
        let range = bar.high - bar.low;
        total_range += range;
    }

    let ambient_noise = if config.window_size > 1 {
        total_range / (config.window_size - 1) as f64
    } else {
        total_range
    };

    let latest_bar = &series.bars[n - 1];
    let latest_range = latest_bar.high - latest_bar.low;

    let current_magnitude = if ambient_noise > 0.0 {
        latest_range / ambient_noise
    } else {
        0.0
    };

    let is_tremor = current_magnitude >= config.tremor_threshold;

    // Calculate a pseudo-Richter scale value
    // log10 of the magnitude, bounded.
    let richter_scale = if current_magnitude > 0.0 {
        current_magnitude.log10() * 3.0 // Scaling factor for effect
    } else {
        0.0
    };

    Ok(SeismicReport {
        ambient_noise,
        current_magnitude,
        is_tremor,
        richter_scale: richter_scale.max(0.0),
    })
}

/// Prints a fun ASCII representation of the seismic report.
pub fn print_ascii_seismology(report: &SeismicReport) {
    println!("=== 🌍 Market Seismology Report ===");
    println!("Ambient Noise Level: {:.4}", report.ambient_noise);
    println!("Current Magnitude:   {:.2}x", report.current_magnitude);

    print!("Richter Scale:       {:.1} ", report.richter_scale);
    let richter_int = report.richter_scale.round() as usize;
    let hashes = "#".repeat(richter_int);
    println!("[{}]", hashes);

    if report.is_tremor {
        println!("⚠️  WARNING: TREMOR DETECTED! ⚠️");
    } else {
        println!("✅ Status: Stable. No significant tremors.");
    }
    println!("===================================");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_mock_bar(high: f64, low: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "test".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 0,
            open: (high + low) / 2.0,
            high,
            low,
            close: (high + low) / 2.0,
            volume: 100.0,
        }
    }

    #[test]
    fn test_analyze_seismology_tremor() {
        let bars = vec![
            create_mock_bar(10.0, 9.0),  // range 1
            create_mock_bar(11.0, 10.0), // range 1
            create_mock_bar(10.5, 9.5),  // range 1
            create_mock_bar(15.0, 5.0),  // range 10 (earthquake!)
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = SeismologyConfig {
            window_size: 4,
            tremor_threshold: 5.0,
        };

        let report = analyze_seismology(&series, config).unwrap();

        assert_eq!(report.ambient_noise, 1.0); // (1+1+1)/3
        assert_eq!(report.current_magnitude, 10.0); // 10 / 1
        assert!(report.is_tremor);
        assert!(report.richter_scale > 0.0);
    }

    #[test]
    fn test_analyze_seismology_stable() {
        let bars = vec![
            create_mock_bar(10.0, 9.0),  // range 1
            create_mock_bar(11.0, 10.0), // range 1
            create_mock_bar(10.5, 9.5),  // range 1
            create_mock_bar(11.5, 10.5), // range 1 (stable)
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = SeismologyConfig {
            window_size: 4,
            tremor_threshold: 5.0,
        };

        let report = analyze_seismology(&series, config).unwrap();

        assert_eq!(report.ambient_noise, 1.0);
        assert_eq!(report.current_magnitude, 1.0);
        assert!(!report.is_tremor);
    }
}
