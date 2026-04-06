#![cfg(feature = "nova")]

use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeDilationReport {
    pub base_volume_rate: f64,
    pub dilation_factor: f64,
}

pub fn analyze_time_dilation(series: &BarSeries) -> Option<TimeDilationReport> {
    if series.bars.is_empty() {
        return None;
    }

    let total_volume: f64 = series.bars.iter().map(|b| b.volume).sum();
    let n = series.bars.len();

    if n == 0 || total_volume == 0.0 {
        return None;
    }

    let base_volume_rate = total_volume / (n as f64);

    // We consider "recent" time as the last ~30% of the series (at least 1 bar)
    let recent_len = (n as f64 * 0.3).ceil() as usize;
    let recent_len = recent_len.max(1);

    let recent_bars = &series.bars[n - recent_len..];
    let recent_volume: f64 = recent_bars.iter().map(|b| b.volume).sum();

    let recent_rate = recent_volume / (recent_len as f64);

    let dilation_factor = recent_rate / base_volume_rate;

    Some(TimeDilationReport {
        base_volume_rate,
        dilation_factor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    #[test]
    fn test_time_dilation_calculates_properly() {
        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![
                Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 1000, open: 10.0, high: 20.0, low: 5.0, close: 15.0, volume: 100.0 },
                Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 2000, open: 15.0, high: 25.0, low: 10.0, close: 20.0, volume: 100.0 },
                Bar { symbol: "TEST".into(), market: "test".into(), timeframe: "1d".into(), timestamp_unix_ms: 3000, open: 20.0, high: 30.0, low: 15.0, close: 25.0, volume: 400.0 },
            ],
        };

        let report = analyze_time_dilation(&series).expect("Should return a report");
        assert_eq!(report.base_volume_rate, 200.0);
        // The last bar is 1/3 (since ceil(3 * 0.3) = 1)
        // Base rate = 600 / 3 = 200.
        // Recent rate = 400 / 1 = 400.
        // Dilation factor = 400 / 200 = 2.0.
        assert_eq!(report.dilation_factor, 2.0);
    }
}
