//! Market Kinematics Analysis
//!
//! This module applies classical mechanics to market data, calculating the "velocity",
//! "acceleration", and "jerk" of price action. By translating price changes into
//! physical motion, traders can detect momentum shifts before they appear on standard indicators.
//!
//! # Why Kinematics?
//! Standard momentum indicators (like RSI or MACD) often lag. By measuring the rate of change
//! of the rate of change (acceleration) and its derivative (jerk), you can anticipate
//! trend exhaustion when acceleration begins to decelerate, even while velocity is still positive.

#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Represents the physical motion characteristics of price action.
///
/// * **Velocity:** The rate of change of price.
/// * **Acceleration:** The rate of change of velocity (momentum strength).
/// * **Jerk:** The rate of change of acceleration (early warning for trend shifts).
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::kinematics::PriceKinematics;
///
/// let kinematics = PriceKinematics {
///     velocity: 15.0,
///     acceleration: 5.0,
///     jerk: -2.0,
/// };
///
/// assert_eq!(kinematics.velocity, 15.0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceKinematics {
    pub velocity: f64,
    pub acceleration: f64,
    pub jerk: f64,
}

/// Calculates the velocity, acceleration, and jerk for a given series of market data.
///
/// Requires at least 4 data points to calculate finite differences up to the third derivative (jerk).
/// Returns `None` if the series contains fewer than 4 bars.
///
/// # Examples
///
/// ```rust
/// use contracts::{Bar, BarSeries};
/// use thales_cli::experimental::kinematics::calculate_kinematics;
///
/// let bars = vec![
///     Bar { symbol: "AAPL".into(), market: "equities".into(), timeframe: "1d".into(), timestamp_unix_ms: 0, open: 100.0, high: 100.0, low: 100.0, close: 100.0, volume: 100.0 },
///     Bar { symbol: "AAPL".into(), market: "equities".into(), timeframe: "1d".into(), timestamp_unix_ms: 1, open: 105.0, high: 105.0, low: 105.0, close: 105.0, volume: 100.0 },
///     Bar { symbol: "AAPL".into(), market: "equities".into(), timeframe: "1d".into(), timestamp_unix_ms: 2, open: 115.0, high: 115.0, low: 115.0, close: 115.0, volume: 100.0 },
///     Bar { symbol: "AAPL".into(), market: "equities".into(), timeframe: "1d".into(), timestamp_unix_ms: 3, open: 130.0, high: 130.0, low: 130.0, close: 130.0, volume: 100.0 },
/// ];
/// let series = BarSeries { schema_version: "v1".into(), bars };
///
/// let kinematics = calculate_kinematics(&series).unwrap();
/// assert_eq!(kinematics.velocity, 15.0);
/// assert_eq!(kinematics.acceleration, 5.0);
/// assert_eq!(kinematics.jerk, 0.0);
/// ```
pub fn calculate_kinematics(series: &BarSeries) -> Option<PriceKinematics> {
    if series.bars.len() < 4 {
        return None;
    }

    let n = series.bars.len();

    // Simple finite differences
    let p0 = series.bars[n - 4].close;
    let p1 = series.bars[n - 3].close;
    let p2 = series.bars[n - 2].close;
    let p3 = series.bars[n - 1].close;

    let v1 = p1 - p0;
    let v2 = p2 - p1;
    let v3 = p3 - p2;

    let a1 = v2 - v1;
    let a2 = v3 - v2;

    let j = a2 - a1;

    Some(PriceKinematics {
        velocity: v3,
        acceleration: a2,
        jerk: j,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_mock_bar(close: f64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "test".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: 0,
            open: close,
            high: close,
            low: close,
            close,
            volume: 100.0,
        }
    }

    #[test]
    fn test_kinematics_calculation() {
        let bars = vec![
            create_mock_bar(100.0), // p0
            create_mock_bar(105.0), // p1 -> v1 = 5
            create_mock_bar(115.0), // p2 -> v2 = 10 -> a1 = 5
            create_mock_bar(130.0), // p3 -> v3 = 15 -> a2 = 5 -> j = 0
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let result = calculate_kinematics(&series).unwrap();

        assert_eq!(result.velocity, 15.0);
        assert_eq!(result.acceleration, 5.0);
        assert_eq!(result.jerk, 0.0);
    }
}
