#![cfg(feature = "nova")]

//! Price DNA Sequence Mapping
//!
//! This module analyzes the sequence of market behavior across time and maps
//! it to a structured biological representation of nucleotides (A, C, T, G).
//! Traditional market indicators analyze values such as moving averages or oscillators.
//! This module instead analyzes the direction of price movements combined with relative volume.
//!
//! Translating market data bars into "Price DNA" exposes the possibility of using
//! powerful, battle-tested bioinformatics algorithms (such as Smith-Waterman or BLAST)
//! to find 'genetic' similarities across different assets or historical epochs.
//!
//! This approach provides traders with a novel tool to discover market turning points
//! based purely on sequences of actions, unlocking deep sequence alignment testing.

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Configuration for analyzing the Price DNA.
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::price_dna::PriceDnaConfig;
///
/// let config = PriceDnaConfig {
///     volume_threshold_multiplier: 1.5,
/// };
///
/// assert_eq!(config.volume_threshold_multiplier, 1.5);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceDnaConfig {
    /// Multiplier to determine if a bar has high or low volume relative to the average.
    /// E.g. A value of 1.5 implies high volume is >= 1.5 * average_volume.
    pub volume_threshold_multiplier: f64,
}

impl Default for PriceDnaConfig {
    fn default() -> Self {
        Self {
            volume_threshold_multiplier: 1.5,
        }
    }
}

/// The output report containing the calculated sequence of biological nucleotides.
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::price_dna::PriceDnaReport;
///
/// let report = PriceDnaReport {
///     symbol: "BTCUSD".to_string(),
///     sequence: "ATCG".to_string(),
///     description: "Legend...".to_string(),
/// };
///
/// assert_eq!(report.sequence, "ATCG");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceDnaReport {
    /// The market asset symbol.
    pub symbol: String,
    /// The sequenced "DNA", composed of A, C, T, G characters.
    pub sequence: String,
    /// A human-readable legend explaining the nucleotide mapping.
    pub description: String,
}

/// Translates market data into a biological sequence based on price direction and volume.
///
/// This sequence mapping algorithm evaluates if a bar's closing price represents an upward or
/// downward move relative to the open, and whether its volume exceeds a defined threshold above
/// the average series volume. The final result is a string of characters composed of A, C, T, G.
///
/// - `A` = Up & High Volume
/// - `C` = Down & High Volume
/// - `T` = Up & Low Volume
/// - `G` = Down & Low Volume
///
/// # Arguments
/// * `series` - The historical [`BarSeries`] data to sequence.
/// * `config` - The [`PriceDnaConfig`] determining the relative volume threshold.
///
/// # Examples
/// ```rust
/// #[cfg(feature = "nova")]
/// # {
/// use contracts::{Bar, BarSeries};
/// use thales_cli::experimental::price_dna::{sequence_dna, PriceDnaConfig};
///
/// let series = BarSeries {
///     schema_version: "v0".to_string(),
///     bars: vec![
///         // Bar 1: Up, Volume: 200 (Above average threshold 125) -> A
///         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 0, open: 100.0, high: 110.0, low: 90.0, close: 105.0, volume: 200.0 },
///         // Bar 2: Down, Volume: 50 (Below average threshold 125) -> G
///         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 1000, open: 100.0, high: 110.0, low: 90.0, close: 95.0, volume: 50.0 },
///     ],
/// };
///
/// let config = PriceDnaConfig {
///     volume_threshold_multiplier: 1.0,
/// };
///
/// let report_opt = sequence_dna(&series, config);
/// assert!(report_opt.is_some());
/// if let Some(report) = report_opt {
///     assert_eq!(report.sequence, "AG");
/// }
/// # }
/// ```
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

/// Prints a visual ASCII representation of the sequenced market DNA.
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
