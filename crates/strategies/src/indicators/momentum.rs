//! Momentum Indicator
//!
//! Calculates the Momentum indicator, which measures the rate of change of a security's price.
//! It compares the current closing price to the closing price from `n` periods ago.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;

/// Calculate Momentum
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with Momentum values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let momentum = strategies::indicators::momentum::calculate(&df, 10)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let close_s = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .cast(&DataType::Float64)?;

    let close_chunked: &Float64Chunked = close_s.f64().context("Close column must be numeric")?;

    let result_chunked: Float64Chunked = close_chunked
        .into_iter()
        .enumerate()
        .map(|(i, opt_curr)| {
            if i < period {
                return None;
            }
            let opt_prev = close_chunked.get(i - period);
            if let (Some(curr_f64), Some(prev_f64)) = (opt_curr, opt_prev) {
                let curr_dec = Decimal::from_f64_retain(curr_f64)?;
                let prev_dec = Decimal::from_f64_retain(prev_f64)?;
                let mom = curr_dec - prev_dec;
                mom.to_f64()
            } else {
                None
            }
        })
        .collect();

    let mut s = result_chunked.into_series();
    s.rename("momentum");
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let mut df = df!(
            "close" => &[10, 11, 12, 13, 15, 14]
        )?;
        df.try_apply("close", |s| s.cast(&DataType::Float64))?;

        // Period 2
        // Mom[0] = None
        // Mom[1] = None
        // Mom[2] = 12.0 - 10.0 = 2.0
        // Mom[3] = 13.0 - 11.0 = 2.0
        // Mom[4] = 15.0 - 12.0 = 3.0
        // Mom[5] = 14.0 - 13.0 = 1.0
        let result = calculate(&df, 2)?;
        let out_chunked: &Float64Chunked = result.f64()?;

        assert_eq!(out_chunked.len(), 6);
        assert!(out_chunked.get(0).is_none());
        assert!(out_chunked.get(1).is_none());

        if let Some(val) = out_chunked.get(2) {
            assert!((val - 2.0).abs() < 1e-10);
        } else {
            anyhow::bail!("Missing value at index 2");
        }

        if let Some(val) = out_chunked.get(3) {
            assert!((val - 2.0).abs() < 1e-10);
        } else {
            anyhow::bail!("Missing value at index 3");
        }

        if let Some(val) = out_chunked.get(4) {
            assert!((val - 3.0).abs() < 1e-10);
        } else {
            anyhow::bail!("Missing value at index 4");
        }

        if let Some(val) = out_chunked.get(5) {
            assert!((val - 1.0).abs() < 1e-10);
        } else {
            anyhow::bail!("Missing value at index 5");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 10);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        // Period > Data length
        let mut df_short = df!("close" => &[10, 11])?;
        df_short.try_apply("close", |s| s.cast(&DataType::Float64))?;
        let res_short = calculate(&df_short, 5)?;
        let out_chunked: &Float64Chunked = res_short.f64()?;
        assert_eq!(out_chunked.len(), 2);
        assert!(out_chunked.get(0).is_none());
        assert!(out_chunked.get(1).is_none());

        // Zero period
        let mut df_normal = df!("close" => &[10, 11])?;
        df_normal.try_apply("close", |s| s.cast(&DataType::Float64))?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());
        if let Err(e) = res_zero {
            assert_eq!(e.to_string(), "Period must be greater than 0");
        }

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<i32> = (0..100).map(|i| 100 + i).collect();
        let mut df = df!("close" => values)?;
        df.try_apply("close", |s| s.cast(&DataType::Float64))?;

        let result = calculate(&df, 14)?;
        assert_eq!(result.len(), 100);

        let out_chunked: &Float64Chunked = result.f64()?;
        assert!(out_chunked.get(13).is_none());
        assert!(out_chunked.get(14).is_some());

        // With an increasing line, momentum over 14 periods should be 14.0
        for i in 14..100 {
            if let Some(val) = out_chunked.get(i) {
                assert!((val - 14.0).abs() < 1e-10);
            } else {
                anyhow::bail!("Missing value at index {}", i);
            }
        }
        Ok(())
    }
}
