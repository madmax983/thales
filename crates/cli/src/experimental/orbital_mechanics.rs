#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbitalMechanics {
    pub eccentric_anomaly: f64,
}

use super::{
    kinematics::calculate_kinematics,
    market_gravity::{MarketGravityConfig, calculate_gravity},
};

pub fn calculate_orbital_mechanics(series: &BarSeries) -> anyhow::Result<Option<OrbitalMechanics>> {
    if series.bars.len() < 4 {
        return Ok(None);
    }

    let config = MarketGravityConfig { num_bins: 10 };
    let gravity = match calculate_gravity(series, config) {
        Some(g) => g,
        None => return Ok(None),
    };

    let kinematics = match calculate_kinematics(series) {
        Some(k) => k,
        None => return Ok(None),
    };

    // Calculate "eccentric anomaly" using distance from Center of Mass and velocity
    let current_price = match series.bars.last() {
        Some(b) => b.close,
        None => 0.0,
    };
    let distance_from_com = (current_price - gravity.center_of_mass).abs();

    // E = distance * velocity (simplified representation for market mechanics)
    let eccentric_anomaly = if gravity.center_of_mass > 0.0 {
        (distance_from_com / gravity.center_of_mass) * kinematics.velocity.abs()
    } else {
        0.0
    };

    Ok(Some(OrbitalMechanics { eccentric_anomaly }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_orbital_mechanics() {
        let bars = vec![
            Bar {
                symbol: "TEST".into(),
                market: "test".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 0,
                open: 10.0,
                high: 20.0,
                low: 5.0,
                close: 15.0,
                volume: 100.0,
            },
            Bar {
                symbol: "TEST".into(),
                market: "test".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 1,
                open: 15.0,
                high: 25.0,
                low: 10.0,
                close: 20.0,
                volume: 200.0,
            },
            Bar {
                symbol: "TEST".into(),
                market: "test".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 2,
                open: 20.0,
                high: 30.0,
                low: 15.0,
                close: 25.0,
                volume: 150.0,
            },
            Bar {
                symbol: "TEST".into(),
                market: "test".into(),
                timeframe: "1d".into(),
                timestamp_unix_ms: 3,
                open: 25.0,
                high: 35.0,
                low: 20.0,
                close: 30.0,
                volume: 100.0,
            },
        ];
        let series = BarSeries {
            schema_version: "v0".into(),
            bars,
        };
        let result = calculate_orbital_mechanics(&series);
        assert!(result.is_ok());
        if let Ok(Some(mech)) = result {
            assert!(mech.eccentric_anomaly > 0.0);
        } else {
            panic!("Expected Some(OrbitalMechanics)");
        }
    }
}
