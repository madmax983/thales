use anyhow::Result;
use polars::prelude::*;
use std::collections::VecDeque;

/// Calculates the Commodity Channel Index (CCI).
///
/// CCI = (Typical Price - SMA(Typical Price)) / (0.015 * Mean Deviation)
/// Typical Price = (High + Low + Close) / 3
/// Mean Deviation = (1/n) * Sum(|Typical Price - SMA(Typical Price)|) over the period.
///
/// # Arguments
///
/// * `df` - The DataFrame containing "high", "low", and "close" columns.
/// * `period` - The period for the calculation.
///
/// # Returns
///
/// A `Result` containing the CCI Series.
pub fn calculate(df: &DataFrame, period: usize) -> Result<Series> {
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let high = df.column("high")?;
    let low = df.column("low")?;
    let close = df.column("close")?;

    // Typical Price (TP) = (High + Low + Close) / 3
    let hl = (high + low)?;
    let hlc = (&hl + close)?;
    let tp_series = &hlc / 3.0;

    // We need to iterate values.
    let tp_len = tp_series.len();
    let tp_f64 = tp_series.f64()?;

    let mut cci_values = Vec::with_capacity(tp_len);
    let mut window: VecDeque<f64> = VecDeque::with_capacity(period);

    let mut sum = 0.0;

    for i in 0..tp_len {
        let val = tp_f64.get(i);

        match val {
            Some(v) => {
                window.push_back(v);
                sum += v;

                if window.len() > period {
                    if let Some(old) = window.pop_front() {
                        sum -= old;
                    }
                }

                if window.len() == period {
                    let sma = sum / period as f64;

                    // Mean Deviation: Sum(|tp_i - sma|) / period
                    let mut sum_abs_diff = 0.0;
                    for &item in &window {
                        sum_abs_diff += (item - sma).abs();
                    }
                    let mean_dev = sum_abs_diff / period as f64;

                    // CCI calculation
                    if mean_dev == 0.0 {
                        cci_values.push(Some(0.0));
                    } else {
                        let cci = (v - sma) / (0.015 * mean_dev);
                        cci_values.push(Some(cci));
                    }
                } else {
                    cci_values.push(None);
                }
            }
            None => {
                window.clear();
                sum = 0.0;
                cci_values.push(None);
            }
        }
    }

    let s = Series::new("cci", cci_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cci_calculation() -> Result<()> {
        // Create a simple DataFrame
        let highs = Series::new("high", &[10.0, 12.0, 15.0, 14.0, 16.0]);
        let lows = Series::new("low", &[8.0, 9.0, 11.0, 10.0, 12.0]);
        let closes = Series::new("close", &[9.0, 11.0, 14.0, 12.0, 15.0]);
        let df = DataFrame::new(vec![highs, lows, closes])?;

        // Period 3
        let cci_series = calculate(&df, 3)?;

        assert_eq!(cci_series.len(), 5);
        // First 2 should be null (period 3 requires 3 values)
        assert!(cci_series.get(0)?.is_null());
        assert!(cci_series.get(1)?.is_null());

        // 3rd element (index 2) should be valid
        assert!(!cci_series.get(2)?.is_null());

        Ok(())
    }

    #[test]
    fn test_cci_values() -> Result<()> {
        // Trend up
        let highs = Series::new("high", &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0]);
        let lows = Series::new("low", &[9.0, 10.0, 11.0, 12.0, 13.0, 14.0]);
        let closes = Series::new("close", &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0]);
        let df = DataFrame::new(vec![highs, lows, closes])?;

        let cci_s = calculate(&df, 3)?;
        // Last element should be valid float
        let last_val = cci_s.get(5)?;

        if let AnyValue::Float64(v) = last_val {
            // In strong uptrend, CCI should be positive
            assert!(v > 0.0, "CCI should be positive in uptrend, got {}", v);
        } else if let AnyValue::Null = last_val {
            panic!("Expected Float64, got Null");
        }

        Ok(())
    }
}
