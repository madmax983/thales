//! Cycle Analysis Module
//!
//! Provides functionality to find dominant cycles in market data using
//! Discrete Fourier Transform (DFT).

use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleMatch {
    pub period: f64,
    pub amplitude: f64,
    pub phase: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleConfig {
    pub max_cycles: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleReport {
    pub symbol: String,
    pub cycles: Vec<CycleMatch>,
}

/// Analyzes dominant cycles using Discrete Fourier Transform (DFT).
pub fn analyze_cycles(series: &BarSeries, config: CycleConfig) -> Result<CycleReport> {
    let bars = &series.bars;
    let n = bars.len();

    if n < 4 {
        return Err(anyhow::anyhow!(
            "Not enough data to find cycles. Need at least 4 bars."
        ));
    }

    // Extract closes
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();

    // Detrend the data (subtract mean)
    let mean = closes.iter().sum::<f64>() / n as f64;
    let detrended: Vec<f64> = closes.iter().map(|&x| x - mean).collect();

    let mut cycles = Vec::new();

    // Compute DFT
    // We only need to check frequencies up to Nyquist (N/2)
    // Avoid k=0 (DC component)
    let half_n = n / 2;
    for k in 1..=half_n {
        let mut real = 0.0;
        let mut imag = 0.0;

        let k_f64 = k as f64;
        let n_f64 = n as f64;

        for (t, &x) in detrended.iter().enumerate() {
            let angle = 2.0 * std::f64::consts::PI * k_f64 * (t as f64) / n_f64;
            real += x * angle.cos();
            imag -= x * angle.sin();
        }

        real /= n_f64;
        imag /= n_f64;

        // Multiply by 2 for positive frequencies
        real *= 2.0;
        imag *= 2.0;

        let amplitude = (real * real + imag * imag).sqrt();
        let phase = imag.atan2(real);
        let period = n_f64 / k_f64;

        cycles.push(CycleMatch {
            period,
            amplitude,
            phase,
        });
    }

    // Sort by amplitude (descending)
    cycles.sort_by(|a, b| {
        b.amplitude
            .partial_cmp(&a.amplitude)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Keep top max_cycles
    cycles.truncate(config.max_cycles);

    Ok(CycleReport {
        symbol: series.bars[0].symbol.clone(),
        cycles,
    })
}

/// Prints a simple ASCII summary of the dominant cycles
pub fn print_ascii_cycles(report: &CycleReport) {
    println!("=== Cycle Analysis Report for {} ===", report.symbol);
    println!("Found {} dominant cycles:", report.cycles.len());

    for (i, c) in report.cycles.iter().enumerate() {
        println!(
            "{}. [Period: {:.2} bars] Amplitude: {:.4}, Phase: {:.4}",
            i + 1,
            c.period,
            c.amplitude,
            c.phase
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_dummy_bar(close: f64, ts: i64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "crypto".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: ts,
            open: close,
            high: close,
            low: close,
            close,
            volume: 100.0,
        }
    }

    #[test]
    fn test_cycle_analysis_sine_wave() {
        // Create a perfect sine wave with period = 10
        let period = 10.0;
        let amplitude = 5.0;

        let mut bars = Vec::new();
        let mut ts = 1000;

        let n = 100;
        for t in 0..n {
            let angle = 2.0 * std::f64::consts::PI * (t as f64) / period;
            let val = amplitude * angle.sin();
            bars.push(create_dummy_bar(100.0 + val, ts));
            ts += 1000;
        }

        let series = BarSeries {
            schema_version: "v1".to_string(),
            bars,
        };

        let config = CycleConfig { max_cycles: 1 };

        let report = analyze_cycles(&series, config).unwrap();

        assert_eq!(report.cycles.len(), 1);
        let dominant = &report.cycles[0];

        // Check period matches
        assert!(
            (dominant.period - period).abs() < 0.1,
            "Expected period ~{}, got {}",
            period,
            dominant.period
        );
        // Check amplitude matches
        assert!(
            (dominant.amplitude - amplitude).abs() < 0.1,
            "Expected amplitude ~{}, got {}",
            amplitude,
            dominant.amplitude
        );
    }
}
