//! Export Utilities
//!
//! This module provides tools to export internal data structures to common formats.
//! For example, it allows exporting `BarSeries` to CSV format for use in external tools
//! like Excel, Python/Pandas, or machine learning pipelines.

use anyhow::Result;
use contracts::BarSeries;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Exports a `BarSeries` to a CSV file.
///
/// This is useful for saving downloaded or generated market data so it can be
/// analyzed using external tools.
///
/// # Arguments
///
/// * `series` - The bar series to export.
/// * `path` - The file path to write the CSV to.
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if the file cannot be created or written to.
pub fn export_bar_series_to_csv(series: &BarSeries, path: impl AsRef<Path>) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(
        file,
        "symbol,market,timeframe,timestamp_unix_ms,open,high,low,close,volume"
    )?;
    for bar in &series.bars {
        writeln!(
            file,
            "{},{},{},{},{},{},{},{},{}",
            bar.symbol,
            bar.market,
            bar.timeframe,
            bar.timestamp_unix_ms,
            bar.open,
            bar.high,
            bar.low,
            bar.close,
            bar.volume
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;
    use std::fs;

    #[test]
    fn test_export_bar_series_to_csv() {
        let bars = vec![
            Bar {
                symbol: "AAPL".to_string(),
                market: "equities".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 1600000000000,
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 102.0,
                volume: 1000.0,
            },
            Bar {
                symbol: "AAPL".to_string(),
                market: "equities".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 1600086400000,
                open: 102.0,
                high: 108.0,
                low: 100.0,
                close: 107.0,
                volume: 2000.0,
            },
        ];
        let series = BarSeries {
            schema_version: "v1".to_string(),
            bars,
        };

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("export_test.csv");

        export_bar_series_to_csv(&series, &file_path).unwrap();

        let contents = fs::read_to_string(&file_path).unwrap();
        let expected_header =
            "symbol,market,timeframe,timestamp_unix_ms,open,high,low,close,volume\n";
        let expected_row1 = "AAPL,equities,1d,1600000000000,100,105,95,102,1000\n";
        let expected_row2 = "AAPL,equities,1d,1600086400000,102,108,100,107,2000\n";

        assert!(contents.contains(expected_header));
        assert!(contents.contains(expected_row1));
        assert!(contents.contains(expected_row2));
    }
}
