//! Sonification Module
//!
//! Provides functionality to convert market data (`BarSeries`) into a musical representation
//! (sonification). By mapping price, volatility, and volume to pitch, velocity, and duration,
//! traders can "hear" the market regime.
//!
//! # Sound Mapping
//! - **Pitch:** Mapped to the closing price (higher price = higher note).
//! - **Velocity (Volume):** Mapped to the bar's volatility (High - Low) (larger range = louder note).
//! - **Duration:** Mapped to the trading volume (higher volume = longer note).

use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Represents a single musical note generated from a price bar.
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::sonification::Note;
///
/// let note = Note {
///     pitch_midi: 60, // Middle C
///     velocity: 100,  // Loud
///     duration_ms: 500, // Half a second
/// };
///
/// assert_eq!(note.pitch_midi, 60);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// The MIDI pitch value (0-127).
    pub pitch_midi: u8,
    /// The velocity or loudness of the note (0-127).
    pub velocity: u8,
    /// The duration of the note in milliseconds.
    pub duration_ms: u32,
}

/// Configuration for the Sonification Analysis.
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::sonification::SonificationConfig;
///
/// let config = SonificationConfig {
///     min_pitch: 48,
///     max_pitch: 84,
/// };
///
/// assert_eq!(config.min_pitch, 48);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonificationConfig {
    /// The minimum MIDI pitch to use (e.g., 48 for C3).
    pub min_pitch: u8,
    /// The maximum MIDI pitch to use (e.g., 84 for C6).
    pub max_pitch: u8,
}

impl Default for SonificationConfig {
    fn default() -> Self {
        Self {
            min_pitch: 48, // C3
            max_pitch: 84, // C6
        }
    }
}

/// The result of a Sonification Analysis run.
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::sonification::{SonificationReport, Note};
///
/// let report = SonificationReport {
///     symbol: "BTCUSD".to_string(),
///     notes: vec![
///         Note { pitch_midi: 60, velocity: 100, duration_ms: 500 }
///     ],
/// };
///
/// assert_eq!(report.symbol, "BTCUSD");
/// assert_eq!(report.notes.len(), 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonificationReport {
    /// The symbol analyzed.
    pub symbol: String,
    /// A list of musical notes representing the market data.
    pub notes: Vec<Note>,
}

/// Converts a `BarSeries` into a sequence of musical notes.
///
/// This function normalizes the price, volatility, and volume across the entire
/// series to map them to MIDI pitches (e.g., 48-84), velocities (e.g., 40-127),
/// and durations (e.g., 100ms - 1000ms).
///
/// # Errors
///
/// Returns an error if the `BarSeries` is empty.
///
/// # Examples
///
/// ```rust
/// use contracts::{BarSeries, Bar};
/// use thales_cli::experimental::sonification::{analyze_sonification, SonificationConfig};
///
/// let bars = vec![
///     Bar {
///         symbol: "TEST".to_string(),
///         market: "equities".to_string(),
///         timeframe: "1d".to_string(),
///         timestamp_unix_ms: 1000,
///         open: 100.0,
///         high: 110.0,
///         low: 90.0,
///         close: 105.0,
///         volume: 1000.0,
///     },
/// ];
///
/// let series = BarSeries {
///     schema_version: "v0".to_string(),
///     bars,
/// };
///
/// let config = SonificationConfig::default();
/// let report = analyze_sonification(&series, config).unwrap();
///
/// assert_eq!(report.notes.len(), 1);
/// ```
pub fn analyze_sonification(
    series: &BarSeries,
    config: SonificationConfig,
) -> Result<SonificationReport> {
    let bars = &series.bars;
    if bars.is_empty() {
        return Err(anyhow::anyhow!("BarSeries is empty. Cannot sonify."));
    }

    let min_p = config.min_pitch.min(config.max_pitch) as f64;
    let max_p = config.max_pitch.max(config.min_pitch) as f64;

    // Find min/max for normalization
    let mut min_close = f64::MAX;
    let mut max_close = f64::MIN;
    let mut max_volatility = f64::MIN;
    let mut min_volatility = f64::MAX;
    let mut max_volume = f64::MIN;
    let mut min_volume = f64::MAX;

    for bar in bars {
        if bar.close < min_close {
            min_close = bar.close;
        }
        if bar.close > max_close {
            max_close = bar.close;
        }

        let volatility = bar.high - bar.low;
        if volatility > max_volatility {
            max_volatility = volatility;
        }
        if volatility < min_volatility {
            min_volatility = volatility;
        }

        if bar.volume > max_volume {
            max_volume = bar.volume;
        }
        if bar.volume < min_volume {
            min_volume = bar.volume;
        }
    }

    // Avoid division by zero
    let close_range = (max_close - min_close).max(1e-9);
    let volatility_range = (max_volatility - min_volatility).max(1e-9);
    let volume_range = (max_volume - min_volume).max(1e-9);

    let mut notes = Vec::with_capacity(bars.len());

    for bar in bars {
        // Pitch: Map close price to [min_pitch, max_pitch]
        let close_norm = (bar.close - min_close) / close_range;
        let pitch = min_p + (close_norm * (max_p - min_p));
        let pitch_midi = pitch.round().clamp(0.0, 127.0) as u8;

        // Velocity: Map volatility to [40, 127]
        let volatility = bar.high - bar.low;
        let vol_norm = (volatility - min_volatility) / volatility_range;
        let velocity = 40.0 + (vol_norm * (127.0 - 40.0));
        let velocity_midi = velocity.round().clamp(0.0, 127.0) as u8;

        // Duration: Map volume to [100ms, 1000ms]
        let volume_norm = (bar.volume - min_volume) / volume_range;
        let duration = 100.0 + (volume_norm * (1000.0 - 100.0));
        let duration_ms = duration.round().max(1.0) as u32;

        notes.push(Note {
            pitch_midi,
            velocity: velocity_midi,
            duration_ms,
        });
    }

    Ok(SonificationReport {
        symbol: bars[0].symbol.clone(),
        notes,
    })
}

/// Prints a simple ASCII representation of the sonified notes.
pub fn print_ascii_sonification(report: &SonificationReport) {
    println!("=== Market Sonification Report for {} ===", report.symbol);
    println!("Generated {} notes.", report.notes.len());

    for (i, note) in report.notes.iter().enumerate() {
        // Visualize pitch with indentation
        let indent = " ".repeat(((note.pitch_midi as f64) / 127.0 * 50.0) as usize);
        let note_symbol = if note.velocity > 100 { "🎵" } else { "♪" };
        println!(
            "{:03}. {}{} [Pitch: {:3}, Vel: {:3}, Dur: {:4}ms]",
            i + 1,
            indent,
            note_symbol,
            note.pitch_midi,
            note.velocity,
            note.duration_ms
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_analyze_sonification_basic() {
        let bars = vec![
            Bar {
                symbol: "TEST".to_string(),
                market: "crypto".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 1000,
                open: 100.0,
                high: 110.0,
                low: 90.0,
                close: 100.0,  // Low close
                volume: 100.0, // Low volume
            },
            Bar {
                symbol: "TEST".to_string(),
                market: "crypto".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 2000,
                open: 100.0,
                high: 150.0,
                low: 50.0,      // High volatility
                close: 200.0,   // High close
                volume: 1000.0, // High volume
            },
        ];

        let series = BarSeries {
            schema_version: "v1".to_string(),
            bars,
        };

        let config = SonificationConfig {
            min_pitch: 48,
            max_pitch: 84,
        };

        let report = analyze_sonification(&series, config).unwrap();

        assert_eq!(report.notes.len(), 2);

        // First bar: min price, min volatility, min volume
        assert_eq!(report.notes[0].pitch_midi, 48); // Min pitch
        assert_eq!(report.notes[0].velocity, 40); // Min velocity
        assert_eq!(report.notes[0].duration_ms, 100); // Min duration

        // Second bar: max price, max volatility, max volume
        assert_eq!(report.notes[1].pitch_midi, 84); // Max pitch
        assert_eq!(report.notes[1].velocity, 127); // Max velocity
        assert_eq!(report.notes[1].duration_ms, 1000); // Max duration
    }

    #[test]
    fn test_analyze_sonification_empty() {
        let series = BarSeries {
            schema_version: "v1".to_string(),
            bars: vec![],
        };

        let result = analyze_sonification(&series, SonificationConfig::default());
        assert!(result.is_err());
    }
}
