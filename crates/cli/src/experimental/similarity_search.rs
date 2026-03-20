//! Similarity Search Module
//!
//! Provides functionality to find historical market patterns similar to a given target window
//! using a normalized Euclidean distance algorithm.

use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityMatch {
    pub start_index: usize,
    pub end_index: usize,
    pub start_timestamp_ms: i64,
    pub end_timestamp_ms: i64,
    pub distance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityConfig {
    pub window_size: usize,
    pub top_k: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityReport {
    pub symbol: String,
    pub target_window_start: i64,
    pub target_window_end: i64,
    pub matches: Vec<SimilarityMatch>,
}

/// Finds the `top_k` most similar historical sequences to the most recent `window_size` bars.
/// Uses a sliding window approach with z-score normalized Euclidean distance.
pub fn find_similar_patterns(
    series: &BarSeries,
    config: SimilarityConfig,
) -> Result<SimilarityReport> {
    let bars = &series.bars;
    let n = bars.len();
    let w = config.window_size;

    if w == 0 {
        return Err(anyhow::anyhow!("Window size must be greater than 0."));
    }

    if n < w * 2 {
        return Err(anyhow::anyhow!(
            "Not enough data to find historical matches. Need at least 2 * window_size bars."
        ));
    }

    // Extract target window (the most recent w bars)
    let target_bars = &bars[n - w..];
    let target_closes: Vec<f64> = target_bars.iter().map(|b| b.close).collect();
    let target_norm = normalize_zscore(&target_closes)?;

    let mut matches = Vec::new();

    // Slide over history, stopping before the target window
    for i in 0..=(n - w * 2) {
        let hist_window = &bars[i..i + w];
        let hist_closes: Vec<f64> = hist_window.iter().map(|b| b.close).collect();

        if let Ok(hist_norm) = normalize_zscore(&hist_closes) {
            let dist = euclidean_distance(&target_norm, &hist_norm);
            matches.push(SimilarityMatch {
                start_index: i,
                end_index: i + w - 1,
                start_timestamp_ms: hist_window[0].timestamp_unix_ms,
                end_timestamp_ms: hist_window[w - 1].timestamp_unix_ms,
                distance: dist,
            });
        }
    }

    // Sort by distance (ascending)
    matches.sort_by(|a, b| {
        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Filter overlapping matches (naive non-maximum suppression)
    let mut filtered_matches = Vec::new();
    for m in matches {
        let overlap = filtered_matches.iter().any(|fm: &SimilarityMatch| {
            // Check if intervals [m.start, m.end] and [fm.start, fm.end] overlap
            m.start_index <= fm.end_index && m.end_index >= fm.start_index
        });
        if !overlap {
            filtered_matches.push(m);
            if filtered_matches.len() == config.top_k {
                break;
            }
        }
    }

    Ok(SimilarityReport {
        symbol: series.bars[0].symbol.clone(),
        target_window_start: target_bars[0].timestamp_unix_ms,
        target_window_end: target_bars[w - 1].timestamp_unix_ms,
        matches: filtered_matches,
    })
}

/// Z-score normalization for a sequence of values
fn normalize_zscore(values: &[f64]) -> Result<Vec<f64>> {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();

    if std_dev == 0.0 {
        return Err(anyhow::anyhow!(
            "Standard deviation is zero, cannot normalize."
        ));
    }

    Ok(values.iter().map(|&x| (x - mean) / std_dev).collect())
}

/// Calculates Euclidean distance between two vectors of the same length
fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// Prints a simple ASCII summary of the similarities
pub fn print_ascii_similarities(report: &SimilarityReport) {
    println!("=== Similarity Search Report for {} ===", report.symbol);
    println!(
        "Target Window: {} to {}",
        format_ts(report.target_window_start),
        format_ts(report.target_window_end)
    );
    println!("Found {} historical matches:", report.matches.len());

    for (i, m) in report.matches.iter().enumerate() {
        println!(
            "{}. [Dist: {:.4}] {} to {}",
            i + 1,
            m.distance,
            format_ts(m.start_timestamp_ms),
            format_ts(m.end_timestamp_ms)
        );
    }
}

fn format_ts(ts: i64) -> String {
    use chrono::DateTime;
    DateTime::from_timestamp_millis(ts)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| ts.to_string())
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
    fn test_similarity_search_exact_match() {
        // Create a pattern
        let pattern = vec![100.0, 110.0, 105.0, 120.0, 115.0];

        // Build a series containing noise, the pattern, more noise, and the pattern again at the end
        let mut bars = Vec::new();
        let mut ts = 1000;

        // Noise
        for _ in 0..10 {
            bars.push(create_dummy_bar(50.0, ts));
            ts += 1000;
        }

        // The historical pattern
        let hist_start_ts = ts;
        for &p in &pattern {
            bars.push(create_dummy_bar(p, ts));
            ts += 1000;
        }
        let hist_end_ts = ts - 1000;

        // More noise
        for _ in 0..10 {
            bars.push(create_dummy_bar(80.0, ts));
            ts += 1000;
        }

        // The target pattern (at the end)
        for &p in &pattern {
            bars.push(create_dummy_bar(p, ts));
            ts += 1000;
        }

        let series = BarSeries {
            schema_version: "v1".to_string(),
            bars,
        };

        let config = SimilarityConfig {
            window_size: 5,
            top_k: 1,
        };

        let report = find_similar_patterns(&series, config).unwrap();

        assert_eq!(report.matches.len(), 1);
        let m = &report.matches[0];
        assert!(m.distance < 1e-6); // Distance should be almost zero
        assert_eq!(m.start_timestamp_ms, hist_start_ts);
        assert_eq!(m.end_timestamp_ms, hist_end_ts);
    }
}
