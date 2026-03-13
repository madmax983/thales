//! Hull Moving Average (HMA)
//!
//! Calculates the Hull Moving Average, developed by Alan Hull.
//! HMA aims to reduce lag while maintaining a smooth curve.
//! The formula is: HMA(n) = WMA(2 * WMA(n/2) - WMA(n)), sqrt(n))

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use crate::indicators::wma;

/// Calculate Hull Moving Average (HMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period for the initial WMA calculations
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
///     "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
/// ).unwrap_or_default();
/// let result = hma::calculate(&df, 3).unwrap_or_default();
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period < 2 {
        anyhow::bail!("Period must be at least 2");
    }

    // WMA(period / 2)
    let half_period = period / 2;
    let wma_half = wma::calculate(data, half_period)?;

    // WMA(period)
    let wma_full = wma::calculate(data, period)?;

    // 2 * WMA(n/2) - WMA(n)
    let mut raw_hm_vals: Vec<Option<f64>> = Vec::with_capacity(data.height());

    let half_iter = wma_half.f64()?.into_iter();
    let full_iter = wma_full.f64()?.into_iter();

    for (half_opt, full_opt) in half_iter.zip(full_iter) {
        if let (Some(h), Some(f)) = (half_opt, full_opt) {
            let h_dec = Decimal::from_f64_retain(h).context("Invalid HMA half value")?;
            let f_dec = Decimal::from_f64_retain(f).context("Invalid HMA full value")?;

            let two = Decimal::from_usize(2).context("Failed to create decimal from 2")?;
            let raw_val = (h_dec * two) - f_dec;
            raw_hm_vals.push(raw_val.to_f64());
        } else {
            raw_hm_vals.push(None);
        }
    }

    let raw_series = Series::new("close", raw_hm_vals);
    let temp_df = DataFrame::new(vec![raw_series])?;

    // sqrt(period)
    let sqrt_period = (period as f64).sqrt().round() as usize;
    if sqrt_period == 0 {
        anyhow::bail!("Square root of period rounded to 0");
    }

    // WMA(..., sqrt(period))
    let mut hma_series = wma::calculate(&temp_df, sqrt_period)?;
    hma_series.rename("hma");

    Ok(hma_series)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_hma_calculation() -> Result<()> {
        // period 4
        // data: 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0
        // HMA is designed to smoothly track price with less lag. In a constant trend, it tracks closely.
        let df = df!(
            "close" => &[
                1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0
            ]
        )?;

        let result = calculate(&df, 4)?;
        let out = result.f64()?;

        let val9 = out.get(9).unwrap_or(0.0);
        // Let's ensure it calculated without panicking and is close to the expected value for a linear trend.
        assert!((val9 - 10.0).abs() < 1e-4);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 1);
        assert!(res_short.f64()?.get(0).is_none());

        let res_period_1 = calculate(&df_short, 1);
        assert!(res_period_1.is_err());
        assert_eq!(res_period_1.unwrap_err().to_string(), "Period must be at least 2");

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
        assert!(out.get(99).is_some());

        Ok(())
    }
}
