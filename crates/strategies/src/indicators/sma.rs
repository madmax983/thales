use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Simple Moving Average (SMA)
///
/// The Simple Moving Average calculates the unweighted mean of the previous `period` data points.
///
/// # Arguments
/// * `data` - DataFrame with a `close` column.
/// * `period` - Lookback period.
///
/// # Returns
/// A `Series` containing the computed SMA values.
///
/// # Panics
/// This function does not panic. Returns an error if the `close` column is missing, the DataFrame is empty, or the period is 0.
///
/// # Edge Cases
/// - **Zero Period:** If `period` is 0, returns an error.
/// - **Insufficient Data:** If the DataFrame has fewer rows than `period`, it will still return a series but values will be `None` until `period` data points are accumulated.
/// - **Missing Data:** If the `close` column contains `NaN` or missing values (`None`), those specific elements will not be included in the sum, which may result in `None` outputs until a full clean window is formed again.
///
/// # Examples
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::sma;
///
/// let df = df!(
///     "close" => &[10.0, 20.0, 30.0, 40.0]
/// ).unwrap();
///
/// let sma_series = sma::calculate(&df, 2).unwrap();
/// let sma_values = sma_series.f64().unwrap();
///
/// assert_eq!(sma_values.get(0), None);
/// assert_eq!(sma_values.get(1), Some(15.0)); // (10 + 20) / 2
/// assert_eq!(sma_values.get(2), Some(25.0)); // (20 + 30) / 2
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
    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let mut sma_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut window: VecDeque<Decimal> = VecDeque::with_capacity(period);
    let mut sum = Decimal::ZERO;
    let period_dec =
        Decimal::from_usize(period).context("Invalid period for Decimal conversion")?;

    for i in 0..close.len() {
        let val_opt = close.get(i);

        match val_opt {
            Some(val) => {
                // Convert to Decimal
                // Handle potential NaN or infinity by treating as None or error?
                // For robustness, let's treat invalid f64 as gap.
                if let Some(d) = Decimal::from_f64_retain(val) {
                    sum += d;
                    window.push_back(d);

                    if window.len() > period {
                        if let Some(old) = window.pop_front() {
                            sum -= old;
                        }
                    }

                    if window.len() == period {
                        let avg = sum / period_dec;
                        sma_values.push(avg.to_f64());
                    } else {
                        sma_values.push(None);
                    }
                } else {
                    // Failed to convert (e.g. NaN)
                    sma_values.push(None);
                    window.clear();
                    sum = Decimal::ZERO;
                }
            }
            None => {
                // Missing data point
                sma_values.push(None);
                window.clear();
                sum = Decimal::ZERO;
            }
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
