#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyConfig {
    pub window: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyReport {
    pub potential_energy: f64,
    pub kinetic_energy: f64,
    pub total_energy: f64,
}

pub fn analyze_energy(series: &BarSeries, config: EnergyConfig) -> anyhow::Result<EnergyReport> {
    if series.bars.len() < config.window || config.window < 2 {
        return Ok(EnergyReport {
            potential_energy: 0.0,
            kinetic_energy: 0.0,
            total_energy: 0.0,
        });
    }

    let n = series.bars.len();

    // Potential Energy ~ Height (Price relative to moving average)
    // Kinetic Energy ~ Velocity squared (Price change squared)

    let mut sum = 0.0;
    for i in (n - config.window)..n {
        sum += series.bars[i].close;
    }
    let sma = sum / config.window as f64;

    let current_price = series.bars[n - 1].close;
    let pe = (current_price - sma).abs() * 9.81; // 9.81 is gravity constant for fun

    let p1 = series.bars[n - 1].close;
    let p0 = series.bars[n - 2].close;
    let v = p1 - p0;
    let mass = series.bars[n - 1].volume; // Volume as mass

    let ke = 0.5 * mass * v * v;

    Ok(EnergyReport {
        potential_energy: pe,
        kinetic_energy: ke,
        total_energy: pe + ke,
    })
}

pub fn print_ascii_energy(report: &EnergyReport) {
    println!("=== Market Energy Physics ===");
    println!("Potential Energy: {:.2}", report.potential_energy);
    println!("Kinetic Energy:   {:.2}", report.kinetic_energy);
    println!("Total Energy:     {:.2}", report.total_energy);
    println!("=============================");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_mock_bar(close: f64, volume: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "test".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 0,
            open: close,
            high: close,
            low: close,
            close,
            volume,
        }
    }

    #[test]
    fn test_analyze_energy() {
        let bars = vec![
            create_mock_bar(100.0, 100.0),
            create_mock_bar(105.0, 100.0),
            create_mock_bar(115.0, 100.0),
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = EnergyConfig { window: 3 };
        let report = analyze_energy(&series, config).unwrap();

        // SMA = (100 + 105 + 115) / 3 = 106.66
        // PE = |115 - 106.66| * 9.81 = 81.75
        // KE = 0.5 * 100 * (115 - 105)^2 = 0.5 * 100 * 100 = 5000

        assert!(report.potential_energy > 0.0);
        assert!(report.kinetic_energy > 0.0);
    }
}
