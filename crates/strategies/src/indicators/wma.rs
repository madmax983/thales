//! Weighted Moving Average (WMA)
//!
//! Calculates the Weighted Moving Average, which places a greater weight on the most recent data points.

use anyhow::{Context, Result};
use polars::prelude::*;

/// Calculate Weighted Moving Average (WMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with WMA values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::wma;
///
/// let df = df!(
///     "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
/// ).unwrap_or_default();
/// let result = wma::calculate(&df, 3).unwrap_or_default();
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Get "close" column
    let close_s = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .cast(&DataType::Float64)?;

    let close = close_s.f64().context("Close column must be numeric")?;

    // Pre-extract into a contiguous Vec, treating non-finite values as missing
    let finite_close: Vec<Option<f64>> = close
        .into_iter()
        .map(|opt_val| opt_val.filter(|v| v.is_finite()))
        .collect();

    let mut wma_values: Vec<Option<f64>> = vec![None; finite_close.len()];

    // Sum of weights: n*(n+1)/2
    let denominator = ((period * (period + 1)) / 2) as f64;

    for (i, wma_out) in wma_values
        .iter_mut()
        .enumerate()
        .take(finite_close.len())
        .skip(period - 1)
    {
        let mut sum = 0.0f64;
        let mut valid = true;

        for j in 0..period {
            // Index of the data point: from oldest to newest in the window
            let idx = i + 1 + j - period;

            // Weight is 1 for the oldest, 2 for the next, ... period for the newest
            let weight = (j + 1) as f64;

            if let Some(d) = finite_close[idx] {
                sum += d * weight;
            } else {
                valid = false;
                break;
            }
        }

        if valid {
            *wma_out = Some(sum / denominator);
        }
    }

    let s = Series::new("wma", wma_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_wma_calculation() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).unwrap_or(0.0);
        assert!((val2 - 11.333333333333334).abs() < 1e-10);

        let val3 = out.get(3).unwrap_or(0.0);
        assert!((val3 - 12.333333333333334).abs() < 1e-10);

        let val4 = out.get(4).unwrap_or(0.0);
        assert!((val4 - 13.333333333333334).abs() < 1e-10);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut closes = Vec::new();
        for i in 0..100 {
            closes.push(100.0 + (i as f64) * 0.5);
        }

        let df = df!("close" => &closes)?;
        let result = calculate(&df, 14)?;

        assert_eq!(result.len(), 100);
        let out = result.f64()?;
        assert!(out.get(12).is_none());
        assert!(out.get(13).is_some());
        assert!(out.get(99).is_some());

        Ok(())
    }
}
