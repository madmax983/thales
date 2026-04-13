#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// 🌟 Nova: Market Fluid Dynamics
/// Analyzes market price and volume movement as a fluid.
/// Measures flow pressure, viscosity (resistance to movement), and turbulence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FluidDynamicsReport {
    pub flow_pressure: f64,
    pub viscosity: f64,
    pub turbulence: f64,
}

pub fn analyze_fluid_dynamics(series: &BarSeries) -> Option<FluidDynamicsReport> {
    if series.bars.len() < 2 {
        return None;
    }

    let mut total_flow_pressure = 0.0;
    let mut total_viscosity = 0.0;
    let mut total_turbulence = 0.0;

    for i in 1..series.bars.len() {
        let prev_bar = &series.bars[i - 1];
        let curr_bar = &series.bars[i];

        let price_change = curr_bar.close - prev_bar.close;
        let true_range = (curr_bar.high - curr_bar.low)
            .max((curr_bar.high - prev_bar.close).abs())
            .max((curr_bar.low - prev_bar.close).abs());

        let volume = curr_bar.volume;

        // Flow Pressure: Directional volume push
        let pressure = if true_range > 0.0 {
            (price_change / true_range) * volume
        } else {
            0.0
        };
        total_flow_pressure += pressure;

        // Viscosity: Volume needed to move price by 1 unit
        let viscosity = if price_change.abs() > 0.0 {
            volume / price_change.abs()
        } else {
            volume // High viscosity if volume didn't move price
        };
        total_viscosity += viscosity;

        // Turbulence: Fluctuation relative to net movement
        let body_range = (curr_bar.open - curr_bar.close).abs();
        let wick_range = true_range - body_range;

        let turbulence = if true_range > 0.0 {
            (wick_range / true_range) * volume
        } else {
            0.0
        };
        total_turbulence += turbulence;
    }

    let n = (series.bars.len() - 1) as f64;

    Some(FluidDynamicsReport {
        flow_pressure: total_flow_pressure / n,
        viscosity: total_viscosity / n,
        turbulence: total_turbulence / n,
    })
}

pub fn print_ascii_fluid_dynamics(report: &FluidDynamicsReport) {
    println!("🌊 Market Fluid Dynamics");
    println!("==========================");
    println!("Flow Pressure : {:.2}", report.flow_pressure);
    println!("Viscosity     : {:.2}", report.viscosity);
    println!("Turbulence    : {:.2}", report.turbulence);
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_fluid_dynamics() {
        let bars = vec![
            Bar {
                symbol: "TEST".to_string(),
                market: "test".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 0,
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 100.0,
                volume: 1000.0,
            },
            Bar {
                symbol: "TEST".to_string(),
                market: "test".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 1,
                open: 100.0,
                high: 110.0,
                low: 100.0,
                close: 110.0,
                volume: 2000.0,
            },
        ];

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let report = analyze_fluid_dynamics(&series).unwrap();

        // price_change = 10.0, true_range = 10.0, volume = 2000.0
        // pressure = (10 / 10) * 2000 = 2000.0
        // viscosity = 2000 / 10 = 200.0
        // body_range = 10.0, wick_range = 0.0, turbulence = 0.0

        assert_eq!(report.flow_pressure, 2000.0);
        assert_eq!(report.viscosity, 200.0);
        assert_eq!(report.turbulence, 0.0);
    }

    #[test]
    fn test_insufficient_bars() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![Bar {
                symbol: "TEST".to_string(),
                market: "test".to_string(),
                timeframe: "1d".to_string(),
                timestamp_unix_ms: 0,
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 100.0,
                volume: 1000.0,
            }],
        };

        assert!(analyze_fluid_dynamics(&series).is_none());
    }
}
