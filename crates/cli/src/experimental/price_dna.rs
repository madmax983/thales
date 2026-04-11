#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceDnaConfig {
    pub volume_threshold_multiplier: f64,
}

impl Default for PriceDnaConfig {
    fn default() -> Self {
        Self {
            volume_threshold_multiplier: 1.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceDnaReport {
    pub symbol: String,
    pub sequence: String,
    pub description: String,
}

/// Translates market data into a biological sequence.
/// A = Up & High Volume
/// C = Down & High Volume
/// T = Up & Low Volume
/// G = Down & Low Volume
pub fn sequence_dna(series: &BarSeries, config: PriceDnaConfig) -> Option<PriceDnaReport> {
    if series.bars.is_empty() {
        return None;
    }

    let n = series.bars.len();
    let mut total_volume = 0.0;
    for bar in &series.bars {
        total_volume += bar.volume;
    }
    let avg_volume = if n > 0 { total_volume / n as f64 } else { 0.0 };
    let high_volume_threshold = avg_volume * config.volume_threshold_multiplier;

    let mut sequence = String::new();
    for bar in &series.bars {
        let is_up = bar.close >= bar.open;
        let is_high_volume = bar.volume >= high_volume_threshold;

        let nucleotide = match (is_up, is_high_volume) {
            (true, true) => 'A',
            (false, true) => 'C',
            (true, false) => 'T',
            (false, false) => 'G',
        };
        sequence.push(nucleotide);
    }

    let description = "A: Up/HighVol, C: Down/HighVol, T: Up/LowVol, G: Down/LowVol".to_string();

    Some(PriceDnaReport {
        symbol: series.bars[0].symbol.clone(),
        sequence,
        description,
    })
}

pub fn print_ascii_dna(report: &PriceDnaReport) {
    println!("=== Price DNA Report ===");
    println!("Symbol: {}", report.symbol);
    println!("Sequence: {}", report.sequence);
    println!("Legend: {}", report.description);
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_sequence_dna() {
        let bars = vec![
            Bar {
                symbol: "TEST".to_string(),
                market: "test".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 0,
                open: 100.0,
                high: 110.0,
                low: 90.0,
                close: 105.0,  // UP
                volume: 200.0, // HIGH VOLUME
            },
            Bar {
                symbol: "TEST".to_string(),
                market: "test".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 1000,
                open: 100.0,
                high: 110.0,
                low: 90.0,
                close: 95.0,  // DOWN
                volume: 50.0, // LOW VOLUME
            },
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        // avg vol = 125. high threshold = 125 * 1.0 = 125.
        let config = PriceDnaConfig {
            volume_threshold_multiplier: 1.0,
        };

        let report = sequence_dna(&series, config).unwrap();
        // Bar 1: Up + High Vol (200 >= 125) -> A
        // Bar 2: Down + Low Vol (50 < 125) -> G
        assert_eq!(report.sequence, "AG");
    }
}
