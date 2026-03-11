//! Hull Moving Average (HMA)
//!
//! Calculates the Hull Moving Average to reduce lag and improve smoothing.

use crate::indicators::wma;
use anyhow::{Context, Result};
use polars::prelude::*;

/// Calculate Hull Moving Average (HMA)
///
/// HMA formula: HMA = WMA(2 * WMA(n/2) - WMA(n), sqrt(n))
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period (n)
///
/// # Returns
/// Series with HMA values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::hma;
///
/// let df = df!(
///     "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
/// ).unwrap_or_default();
/// let result = hma::calculate(&df, 4).unwrap_or_default();
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }
    if period == 1 {
        // HMA(1) is just the close price
        let close_s = data
            .column("close")
            .context("DataFrame must contain 'close' column")?
            .cast(&DataType::Float64)?;
        let mut cloned_s = close_s.clone();
        cloned_s.rename("hma");
        return Ok(cloned_s);
    }

    let half_period = period / 2;
    let sqrt_period = (period as f64).sqrt().round() as usize;
    let sqrt_period = if sqrt_period == 0 { 1 } else { sqrt_period };

    let wma_half = wma::calculate(data, half_period)?;
    let wma_full = wma::calculate(data, period)?;

    let wma_half_f64 = wma_half.f64()?;
    let wma_full_f64 = wma_full.f64()?;

    let mut raw_hma_values: Vec<Option<f64>> = Vec::with_capacity(wma_half_f64.len());

    for i in 0..wma_half_f64.len() {
        match (wma_half_f64.get(i), wma_full_f64.get(i)) {
            (Some(half), Some(full)) => {
                raw_hma_values.push(Some(2.0 * half - full));
            }
            _ => {
                raw_hma_values.push(None);
            }
        }
    }

    let raw_hma_series = Series::new("close", raw_hma_values);
    let temp_df = DataFrame::new(vec![raw_hma_series])?;

    let mut final_hma = wma::calculate(&temp_df, sqrt_period)?;
    final_hma.rename("hma");

    Ok(final_hma)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_hma_calculation() -> Result<()> {
        // Mock data to test HMA
        // For a period of 4:
        // half_period = 2
        // sqrt_period = 2
        // WMA(2) and WMA(4) need to be calculated
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0]
        )?;

        let result = calculate(&df, 4)?;
        let out = result.f64()?;

        // Need at least period-1 elements for wma full + sqrt_period-1 for the outer wma
        // WMA(4) needs 4 elements (starts at index 3)
        // outer WMA(2) needs 2 elements (starts at index 4)
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());
        assert!(out.get(3).is_none());

        assert!(out.get(4).is_some());
        assert!(out.get(5).is_some());

        Ok(())
    }

    #[test]
    fn test_hma_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        // Test period = 0
        let res_zero = calculate(&df_short, 0);
        assert!(res_zero.is_err());

        // Test period = 1
        let df_one = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_one = calculate(&df_one, 1)?;
        assert_eq!(res_one.len(), 3);
        assert_eq!(res_one.f64()?.get(0).unwrap_or(0.0), 10.0);
        assert_eq!(res_one.f64()?.get(1).unwrap_or(0.0), 11.0);

        Ok(())
    }
}
