#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Kinematic traits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceKinematics {
    pub velocity: f64,
    pub acceleration: f64,
    pub jerk: f64,
}

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
