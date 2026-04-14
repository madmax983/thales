#![cfg(feature = "nova")]

//! Market Immunology Module
//!
//! This module analyzes the "immune system" of a market. It measures how quickly
//! an asset recovers from sudden, sharp drawdowns (price shocks or "infections").
//!
//! A market with a strong immune system bounces back rapidly after a shock, indicating
//! strong underlying demand and resilience. A market with a weak immune system
//! languishes or continues to bleed after a drop, indicating exhaustion or fear.
//!
//! By calculating an `immunity_score`, traders can identify robust assets to buy
//! during dips or weak assets to short on rallies.

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Configuration for the Market Immunology analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmunologyConfig {
    /// The threshold percentage drop (e.g., 2.0 for a 2% drop) to be considered a "shock".
    pub shock_threshold_pct: f64,
    /// The number of bars to observe after a shock to measure the recovery.
    pub recovery_window: usize,
}

impl Default for ImmunologyConfig {
    fn default() -> Self {
        Self {
            shock_threshold_pct: 2.0, // 2% drop is a shock
            recovery_window: 5,       // Look 5 bars ahead to measure recovery
        }
    }
}

/// The result of the Market Immunology analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmunologyReport {
    /// The total number of price shocks detected.
    pub total_shocks: usize,
    /// The average percentage of the shock that was recovered within the window.
    /// (e.g., 100.0 means fully recovered, 50.0 means half recovered).
    pub average_recovery_pct: f64,
    /// The final computed immunity score (0-100 scale). Higher is better.
    pub immunity_score: f64,
    /// A human-readable assessment of the market's health.
    pub health_assessment: String,
}

/// Analyzes a `BarSeries` to determine its Market Immunology score.
///
/// This function scans the historical data for sudden drops (shocks) that exceed
/// the `shock_threshold_pct`. For each shock found, it looks ahead by `recovery_window`
/// bars to measure how much of the initial drop was recovered.
///
/// The final `immunity_score` aggregates these recoveries into a single metric.
///
/// # Arguments
/// * `series` - The historical [`BarSeries`] data.
/// * `config` - The [`ImmunologyConfig`] defining shock and recovery parameters.
///
/// # Examples
/// ```rust
/// #[cfg(feature = "nova")]
/// # {
/// use contracts::{Bar, BarSeries};
/// use thales_cli::experimental::market_immunology::{analyze_immunology, ImmunologyConfig};
///
/// let series = BarSeries {
///     schema_version: "v0".to_string(),
///     bars: vec![
///         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 0, open: 100.0, high: 100.0, low: 90.0, close: 90.0, volume: 100.0 }, // Shock (-10%)
///         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 1000, open: 90.0, high: 95.0, low: 90.0, close: 95.0, volume: 100.0 },
///         Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 2000, open: 95.0, high: 100.0, low: 95.0, close: 100.0, volume: 100.0 }, // Fully recovered
///     ],
/// };
///
/// let config = ImmunologyConfig { shock_threshold_pct: 5.0, recovery_window: 2 };
/// let report_opt = analyze_immunology(&series, config);
/// assert!(report_opt.is_some());
/// if let Some(report) = report_opt {
///     assert_eq!(report.total_shocks, 1);
///     assert!(report.average_recovery_pct >= 100.0);
/// }
/// # }
/// ```
pub fn analyze_immunology(
    series: &BarSeries,
    config: ImmunologyConfig,
) -> Option<ImmunologyReport> {
    if series.bars.is_empty() || config.recovery_window == 0 {
        return None;
    }

    let mut total_shocks = 0;
    let mut cumulative_recovery_pct = 0.0;
    let bars = &series.bars;

    for i in 0..bars.len() {
        let bar = &bars[i];
        let open = bar.open;
        let close = bar.close;

        // Check for a shock drop (open -> close)
        if open > 0.0 && close < open {
            let drop_pct = ((open - close) / open) * 100.0;

            if drop_pct >= config.shock_threshold_pct {
                total_shocks += 1;

                // Measure recovery over the window
                let shock_amount_abs = open - close;
                let mut max_recovery_price = close;

                let window_end = (i + 1 + config.recovery_window).min(bars.len());
                for future_bar in bars.iter().take(window_end).skip(i + 1) {
                    if future_bar.high > max_recovery_price {
                        max_recovery_price = future_bar.high;
                    }
                }

                let amount_recovered = max_recovery_price - close;
                let recovery_pct = if shock_amount_abs > 0.0 {
                    (amount_recovered / shock_amount_abs) * 100.0
                } else {
                    0.0
                };

                // Cap individual recovery at 150% to prevent massive spikes from skewing
                cumulative_recovery_pct += recovery_pct.clamp(0.0, 150.0);
            }
        }
    }

    if total_shocks == 0 {
        return Some(ImmunologyReport {
            total_shocks: 0,
            average_recovery_pct: 0.0,
            immunity_score: 100.0, // Never gets shocked = perfect immunity
            health_assessment: "Perfectly Healthy (No shocks detected)".to_string(),
        });
    }

    let average_recovery_pct = cumulative_recovery_pct / (total_shocks as f64);

    // Score mapping: 100% recovery = 100 score. 50% = 50. 0% = 0.
    let immunity_score = average_recovery_pct.clamp(0.0, 100.0);

    let health_assessment = if immunity_score >= 80.0 {
        "Ironclad Immune System 🛡️".to_string()
    } else if immunity_score >= 50.0 {
        "Recovering (Fighting off infection) 🩹".to_string()
    } else if immunity_score >= 20.0 {
        "Compromised Immune System 🤒".to_string()
    } else {
        "Critical Condition (Failing to recover) 💀".to_string()
    };

    Some(ImmunologyReport {
        total_shocks,
        average_recovery_pct,
        immunity_score,
        health_assessment,
    })
}

/// Prints a visual ASCII representation of the immunology report.
pub fn print_ascii_immunology(report: &ImmunologyReport) {
    println!("=== 🦠 Market Immunology Report ===");
    println!("Total Price Shocks:  {}", report.total_shocks);
    println!("Average Recovery:    {:.2}%", report.average_recovery_pct);

    print!("Immunity Score:      {:.1} ", report.immunity_score);
    let bar_len = (report.immunity_score / 5.0) as usize; // 20 chars max
    let bar = "█".repeat(bar_len);
    println!("[{}]", bar);

    println!("Health Assessment:   {}", report.health_assessment);
    println!("===================================");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_mock_bar(open: f64, high: f64, close: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "test".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 0,
            open,
            high,
            low: close.min(open),
            close,
            volume: 100.0,
        }
    }

    #[test]
    fn test_immunology_strong_recovery() {
        let bars = vec![
            create_mock_bar(100.0, 100.0, 90.0), // Shock: -10%
            create_mock_bar(90.0, 95.0, 95.0),   // Recovery starts
            create_mock_bar(95.0, 100.0, 100.0), // Full recovery
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = ImmunologyConfig {
            shock_threshold_pct: 5.0,
            recovery_window: 2,
        };

        let report = analyze_immunology(&series, config).unwrap();
        assert_eq!(report.total_shocks, 1);
        assert!(report.average_recovery_pct >= 100.0);
        assert_eq!(report.immunity_score, 100.0);
        assert!(report.health_assessment.contains("Ironclad"));
    }

    #[test]
    fn test_immunology_weak_recovery() {
        let bars = vec![
            create_mock_bar(100.0, 100.0, 80.0), // Shock: -20%
            create_mock_bar(80.0, 82.0, 81.0),   // Weak recovery
            create_mock_bar(81.0, 83.0, 80.0),   // Fails to bounce
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = ImmunologyConfig {
            shock_threshold_pct: 10.0,
            recovery_window: 2,
        };

        let report = analyze_immunology(&series, config).unwrap();
        assert_eq!(report.total_shocks, 1);
        // Dropped 20 points. Max high was 83, so recovered 3 points. 3/20 = 15%.
        assert_eq!(report.average_recovery_pct, 15.0);
        assert_eq!(report.immunity_score, 15.0);
        assert!(report.health_assessment.contains("Critical"));
    }

    #[test]
    fn test_immunology_no_shocks() {
        let bars = vec![
            create_mock_bar(100.0, 102.0, 101.0),
            create_mock_bar(101.0, 105.0, 104.0),
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = ImmunologyConfig::default();
        let report = analyze_immunology(&series, config).unwrap();
        assert_eq!(report.total_shocks, 0);
        assert_eq!(report.immunity_score, 100.0);
    }
}
