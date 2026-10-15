use anyhow::{Context, Result};
use polars::prelude::*;
use std::collections::VecDeque;

/// Calculate Simple Moving Average (SMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with SMA values.
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

    let mut sma_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let close = super::values_or_nan(close);
    let mut window: VecDeque<f64> = VecDeque::with_capacity(period);
    let mut sum = 0.0f64;
    let period_f = period as f64;

    for &val in close.iter() {
        // Null, NaN and infinity all count as a gap.
        if val.is_finite() {
            sum += val;
            window.push_back(val);

            if window.len() > period
                && let Some(old) = window.pop_front()
            {
                sum -= old;
            }

            if window.len() == period {
                let avg = sum / period_f;
                sma_values.push(Some(avg));
            } else {
                sma_values.push(None);
            }
        } else {
            sma_values.push(None);
            window.clear();
            sum = 0.0;
        }
    }

    let s = Series::new("sma", sma_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[1.0, 2.0, 3.0, 4.0, 5.0]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        // Expected: [null, null, 2.0, 3.0, 4.0]
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert_eq!(out.get(2), Some(2.0));
        assert_eq!(out.get(3), Some(3.0));
        assert_eq!(out.get(4), Some(4.0));

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!("close" => &[10.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 1);
        assert!(res_short.f64()?.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[100.0, 102.0, 101.0, 103.0, 102.0]
        )?;
        // Period 2
        // [null, 101.0, 101.5, 102.0, 102.5]
        let s = calculate(&df, 2)?;
        let out = s.f64()?;

        assert!(out.get(0).is_none());
        assert_eq!(out.get(1), Some(101.0));
        assert_eq!(out.get(2), Some(101.5));
        assert_eq!(out.get(3), Some(102.0));
        assert_eq!(out.get(4), Some(102.5));

        Ok(())
    }
}
