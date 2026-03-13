//! Hull Moving Average (HMA)
//!
//! Calculates the Hull Moving Average, which aims to reduce the lag of a traditional
//! moving average while increasing its responsiveness.
//!
//! Formula: HMA = WMA(2 * WMA(n/2) - WMA(n), sqrt(n))

use crate::indicators::wma;
use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

/// Calculate Hull Moving Average (HMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with HMA values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use rust_decimal::Decimal;
/// use strategies::indicators::hma;
///
/// let df = df!(
///     "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0]
/// ).unwrap();
/// let result = hma::calculate(&df, 4).unwrap();
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period < 2 {
        anyhow::bail!("Period must be at least 2");
    }

    // Step 1: Calculate WMA(n/2)
    let half_period = period / 2;
    let wma_half = wma::calculate(data, half_period).context("Failed to calculate WMA(n/2)")?;

    // Step 2: Calculate WMA(n)
    let wma_full = wma::calculate(data, period).context("Failed to calculate WMA(n)")?;

    // Step 3: Calculate 2 * WMA(n/2) - WMA(n)

    let wma_half_arr = wma_half.f64()?;
    let wma_full_arr = wma_full.f64()?;

    let mut diff_values: Vec<Option<f64>> = vec![None; data.height()];
    for (i, diff_out) in diff_values.iter_mut().enumerate().take(data.height()) {
        if let (Some(half_val), Some(full_val)) = (wma_half_arr.get(i), wma_full_arr.get(i)) {
            let half_dec = Decimal::from_f64_retain(half_val).unwrap_or(Decimal::ZERO);
            let full_dec = Decimal::from_f64_retain(full_val).unwrap_or(Decimal::ZERO);
            let two = Decimal::from_usize(2).unwrap_or(Decimal::ONE);

            let diff = (half_dec * two) - full_dec;
            *diff_out = diff.to_f64();
        }
    }

    let diff_series = Series::new("close", diff_values);
    let temp_df = DataFrame::new(vec![diff_series])?;

    // Step 4: Calculate WMA(sqrt(n)) on the difference
    let sqrt_period = (period as f64).sqrt().round() as usize;
    if sqrt_period == 0 {
        anyhow::bail!("Invalid sqrt_period");
    }

    let hma_result =
        wma::calculate(&temp_df, sqrt_period).context("Failed to calculate final WMA for HMA")?;

    // Rename to match expected output name
    let mut out_series = hma_result.clone();
    out_series.rename("hma");

    Ok(out_series)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_hma_calculation() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0]
        )?;

        let result = calculate(&df, 4)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());
        assert!(out.get(3).is_none()); // 4-period WMA + 2-period WMA(sqrt(4)=2) - 1 lag = 4

        if let Some(val4) = out.get(4) {
            assert!((val4 - 14.0).abs() < 1e-5);
        } else {
            anyhow::bail!("val4 missing");
        }

        if let Some(val5) = out.get(5) {
            assert!((val5 - 15.0).abs() < 1e-5);
        } else {
            anyhow::bail!("val5 missing");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        let df_normal = df!("close" => &[10.0])?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());
        if let Err(e) = res_zero {
            assert_eq!(e.to_string(), "Period must be at least 2");
        }

        let res_one = calculate(&df_normal, 1);
        assert!(res_one.is_err());

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 4)?;
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

        // HMA(14) uses WMA(7), WMA(14) -> diff
        // diff length = 100
        // then WMA(sqrt(14) = 4) on diff
        assert!(out.get(99).is_some());

        Ok(())
    }
}
