use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermodynamicsConfig {
    pub window_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermodynamicsReport {
    pub temperature: f64,
    pub pressure: f64,
    pub energy: f64,
}

pub fn analyze_thermodynamics(series: &BarSeries, config: ThermodynamicsConfig) -> Result<ThermodynamicsReport> {
    let bars = &series.bars;
    let n = bars.len();

    if n < config.window_size || config.window_size == 0 {
        anyhow::bail!("Not enough data for thermodynamics analysis");
    }

    let mut total_temperature = 0.0;
    let mut total_pressure = 0.0;
    let mut total_energy = 0.0;

    let window_bars = &bars[n - config.window_size..];

    for bar in window_bars {
        let volatility = if bar.low > 0.0 { (bar.high - bar.low) / bar.low } else { 0.0 };
        total_temperature += volatility;
        total_pressure += bar.volume;
        let momentum = if bar.open > 0.0 { (bar.close - bar.open).abs() / bar.open } else { 0.0 };
        total_energy += momentum;
    }

    let temperature = total_temperature / config.window_size as f64;
    let pressure = total_pressure / config.window_size as f64;
    let energy = total_energy / config.window_size as f64;

    Ok(ThermodynamicsReport {
        temperature,
        pressure,
        energy,
    })
}

pub fn print_ascii_thermodynamics(report: &ThermodynamicsReport) {
    println!("=== Market Thermodynamics ===");
    println!("Temperature (Volatility): {:.4}", report.temperature);
    println!("Pressure (Volume):        {:.2}", report.pressure);
    println!("Energy (Momentum):        {:.4}", report.energy);
    println!("=============================");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_thermodynamics() {
        let bars = vec![
            Bar { symbol: "TEST".to_string(), market: "crypto".to_string(), timeframe: "1d".to_string(), timestamp_unix_ms: 1000, open: 100.0, high: 110.0, low: 90.0, close: 105.0, volume: 1000.0 },
            Bar { symbol: "TEST".to_string(), market: "crypto".to_string(), timeframe: "1d".to_string(), timestamp_unix_ms: 2000, open: 105.0, high: 115.0, low: 100.0, close: 110.0, volume: 1500.0 },
        ];
        let series = BarSeries { schema_version: "v0".to_string(), bars };
        let config = ThermodynamicsConfig { window_size: 2 };
        let result = analyze_thermodynamics(&series, config);

        match result {
            Ok(report) => {
                assert!(report.temperature > 0.0);
                assert!(report.pressure > 0.0);
                assert!(report.energy > 0.0);
            }
            Err(e) => panic!("Expected Ok, got Err: {}", e),
        }
    }
}
