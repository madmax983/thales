//! True Range (TR)
//!
//! The True Range indicator is a measure of market volatility.
//! It is defined as the greatest of the following:
//! - Current High less the Current Low
//! - Absolute value of the Current High less the Previous Close
//! - Absolute value of the Current Low less the Previous Close

use anyhow::{Context, Result};
use polars::prelude::*;

use rust_decimal::Decimal;
use std::str::FromStr;

/// Calculate True Range (TR)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", and "close" columns (must be castable to String)
///
/// # Returns
/// Series with TR values as strings to maintain Decimal precision.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::true_range;
///
/// fn example() -> anyhow::Result<()> {
///     let df = df!(
///         "high" => &["10", "12", "15"],
///         "low" => &["8", "9", "11"],
///         "close" => &["9", "11", "14"]
///     )?;
///     let mut df = df.clone();
///     df.try_apply("high", |s| s.cast(&DataType::String))?;
///     df.try_apply("low", |s| s.cast(&DataType::String))?;
///     df.try_apply("close", |s| s.cast(&DataType::String))?;
///     let result = true_range::calculate(&df)?;
///     Ok(())
/// }
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    let high_series = data.column("high").context("Missing 'high' column")?;
    let low_series = data.column("low").context("Missing 'low' column")?;
    let close_series = data.column("close").context("Missing 'close' column")?;

    // Cast inputs to String to avoid any f64 float precision loss at the boundary
    let high_arr = high_series.cast(&DataType::String)?;
    let high_chunked = high_arr.str()?;

    let low_arr = low_series.cast(&DataType::String)?;
    let low_chunked = low_arr.str()?;

    let close_arr = close_series.cast(&DataType::String)?;
    let close_chunked = close_arr.str()?;

    let mut tr_values: Vec<Option<String>> = Vec::with_capacity(data.height());

    let mut prev_close_opt: Option<Decimal> = None;

    for ((h_opt, l_opt), c_opt) in high_chunked
        .into_iter()
        .zip(low_chunked.into_iter())
        .zip(close_chunked.into_iter())
    {
        let curr_close = c_opt.and_then(|c| Decimal::from_str(c).ok());

        if let (Some(h_str), Some(l_str)) = (h_opt, l_opt) {
            if let (Ok(h_dec), Ok(l_dec)) = (Decimal::from_str(h_str), Decimal::from_str(l_str)) {
                let mut tr = h_dec - l_dec;

                if let Some(pc_dec) = prev_close_opt {
                    let hpc = (h_dec - pc_dec).abs();
                    let lpc = (l_dec - pc_dec).abs();

                    tr = tr.max(hpc).max(lpc);
                }

                tr_values.push(Some(tr.to_string()));
            } else {
                tr_values.push(None);
            }
        } else {
            tr_values.push(None);
        }

        prev_close_opt = curr_close;
    }

    let mut result_series = Series::new("true_range", tr_values);
    result_series.rename("true_range");

    Ok(result_series)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" => &["10.0", "12.0", "15.0", "14.0"],
            "low" => &["8.0", "9.0", "11.0", "10.0"],
            "close" => &["9.0", "11.0", "14.0", "12.0"]
        )?;

        let result = calculate(&df)?;
        assert_eq!(result.len(), 4);

        let res_arr = result.str()?;

        // Index 0: High - Low = 10.0 - 8.0 = 2.0
        if let Some(val) = res_arr.get(0) {
            let val_dec = Decimal::from_str(val).context("parse decimal")?;
            assert_eq!(val_dec, Decimal::from_str("2.0")?);
        } else {
            anyhow::bail!("Value at index 0 should not be None");
        }

        // Index 1: High=12.0, Low=9.0, PrevClose=9.0
        // TR = max(12-9, |12-9|, |9-9|) = max(3, 3, 0) = 3.0
        if let Some(val) = res_arr.get(1) {
            let val_dec = Decimal::from_str(val).context("parse decimal")?;
            assert_eq!(val_dec, Decimal::from_str("3.0")?);
        } else {
            anyhow::bail!("Value at index 1 should not be None");
        }

        // Index 2: High=15.0, Low=11.0, PrevClose=11.0
        // TR = max(15-11, |15-11|, |11-11|) = max(4, 4, 0) = 4.0
        if let Some(val) = res_arr.get(2) {
            let val_dec = Decimal::from_str(val).context("parse decimal")?;
            assert_eq!(val_dec, Decimal::from_str("4.0")?);
        } else {
            anyhow::bail!("Value at index 2 should not be None");
        }

        // Index 3: High=14.0, Low=10.0, PrevClose=14.0
        // TR = max(14-10, |14-14|, |10-14|) = max(4, 0, 4) = 4.0
        if let Some(val) = res_arr.get(3) {
            let val_dec = Decimal::from_str(val).context("parse decimal")?;
            assert_eq!(val_dec, Decimal::from_str("4.0")?);
        } else {
            anyhow::bail!("Value at index 3 should not be None");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        let df_single = df!(
            "high" => &["10.0"],
            "low" => &["8.0"],
            "close" => &["9.0"]
        )?;

        let res_single = calculate(&df_single)?;
        let out_single = res_single.str()?;
        assert_eq!(out_single.len(), 1);
        if let Some(val) = out_single.get(0) {
            let val_dec = Decimal::from_str(val).context("parse decimal")?;
            assert_eq!(val_dec, Decimal::from_str("2.0")?);
        } else {
            anyhow::bail!("Value at index 0 should not be None");
        }

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "high" => &["105.0", "108.0", "107.0", "110.0", "109.0"],
            "low" => &["100.0", "102.0", "103.0", "105.0", "104.0"],
            "close" => &["103.0", "106.0", "104.0", "108.0", "107.0"]
        )?;

        let result = calculate(&df);
        assert!(result.is_ok());
        let s = result?;
        assert_eq!(s.len(), 5);

        let out = s.str()?;

        // Validation for the 4th index (index 3)
        // High = 110, Low = 105, PrevClose = 104
        // hl = 5, hpc = 6, lpc = 1
        // tr = 6
        if let Some(val) = out.get(3) {
            let val_dec = Decimal::from_str(val).context("parse decimal")?;
            assert_eq!(val_dec, Decimal::from_str("6.0")?);
        } else {
            anyhow::bail!("Value at index 3 should not be None");
        }

        Ok(())
    }
}
