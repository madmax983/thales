use anyhow::{Context, Result};
use polars::prelude::*;
use std::collections::VecDeque;

/// Calculate Z-Score
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period for SMA and Standard Deviation
///
/// # Returns
/// Series with Z-Score values.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Get "close" column
    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let mut zscore_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut window: VecDeque<f64> = VecDeque::with_capacity(period);

    let period_f = period as f64;

    for i in 0..close.len() {
        match close.get(i).filter(|v| v.is_finite()) {
            Some(d) => {
                window.push_back(d);
                if window.len() > period {
                    window.pop_front();
                }

                if window.len() == period {
                    // Two-pass mean/variance per window: no rolling-sum cancellation drift.
                    let mean = window.iter().sum::<f64>() / period_f;
                    let variance =
                        window.iter().map(|&x| (x - mean) * (x - mean)).sum::<f64>() / period_f;
                    let std_dev = variance.sqrt();

                    if std_dev == 0.0 {
                        zscore_values.push(Some(0.0));
                    } else {
                        zscore_values.push(Some((d - mean) / std_dev));
                    }
                } else {
                    // Not enough data yet
                    zscore_values.push(None);
                }
            }
            None => {
                // Missing or non-finite data: reset
                window.clear();
                zscore_values.push(None);
            }
        }
    }

    let s = Series::new("zscore", zscore_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df_stable = df!(
            "close" => &[10.0, 10.0, 10.0, 10.0, 10.0]
        )?;
        let result = calculate(&df_stable, 3)?;
        let z_vals = result.f64()?;

        assert!(z_vals.get(0).is_none());
        assert!(z_vals.get(1).is_none());
        assert_eq!(z_vals.get(2), Some(0.0));
        assert_eq!(z_vals.get(3), Some(0.0));

        let df_var = df!(
            "close" => &[10.0, 12.0, 14.0, 16.0, 18.0]
        )?;
        let result_v = calculate(&df_var, 3)?;
        let z_v_vals = result_v.f64()?;

        // Index 2: [10, 12, 14]. Mean = 12. StdDev = sqrt(2.666...) = 1.63299
        // d = 14
        // Z = (14 - 12) / 1.63299 = 2 / 1.63299 = 1.2247448...
        let val = z_v_vals.get(2).unwrap();
        assert!((val - 1.2247448).abs() < 0.0001);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0, 11.0])?;
        let result = calculate(&df_short, 5)?;
        assert!(result.f64()?.get(0).is_none());
        assert!(result.f64()?.get(1).is_none());

        Ok(())
    }
}
