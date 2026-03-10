//! Weighted Moving Average (WMA)
//!
//! Calculates the Weighted Moving Average of price data over a specified lookback period.
//! WMA places a greater weight on the most recent data points.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate WMA
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - The size of the moving window (must be > 0)
///
/// # Returns
/// Series with WMA values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let wma_series = strategies::indicators::wma::calculate(&df, 14)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let mut wma_values = Vec::with_capacity(close.len());
    let weight_sum = (period * (period + 1)) / 2;
    let weight_sum_dec = Decimal::from(weight_sum);

    let close_vec: Vec<Option<f64>> = close.into_iter().collect();

    for i in 0..close_vec.len() {
        if i < period - 1 {
            wma_values.push(None);
            continue;
        }

        let mut sum = Decimal::ZERO;
        let mut valid_window = true;

        for j in 0..period {
            let idx = i + 1 + j - period;
            if let Some(val) = close_vec[idx] {
                if let Some(val_dec) = Decimal::from_f64_retain(val) {
        let weight = Decimal::from(j + 1);
                    sum += val_dec * weight;
                } else {
                    valid_window = false;
                    break;
                }
            } else {
                valid_window = false;
                break;
            }
        }

        if valid_window && !weight_sum_dec.is_zero() {
            let wma = sum / weight_sum_dec;
            wma_values.push(wma.to_f64());
        } else {
            wma_values.push(None);
        }
    }

    Ok(Series::new("wma", wma_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!("close" => &[10.0, 11.0, 12.0, 13.0, 14.0])?;
        // period 3
        // weights: 1, 2, 3 (sum = 6)
        // idx 2: (10*1 + 11*2 + 12*3) / 6 = (10 + 22 + 36) / 6 = 68 / 6 = 11.333333
        // idx 3: (11*1 + 12*2 + 13*3) / 6 = (11 + 24 + 39) / 6 = 74 / 6 = 12.333333
        // idx 4: (12*1 + 13*2 + 14*3) / 6 = (12 + 26 + 42) / 6 = 80 / 6 = 13.333333

        let result = calculate(&df, 3)?;
        let s = result.f64()?;

        assert!(s.get(0).is_none());
        assert!(s.get(1).is_none());
        if let Some(val) = s.get(2) {
            assert!((val - 11.333333).abs() < 1e-5);
        } else {
            anyhow::bail!("Expected value at index 2");
        }
        if let Some(val) = s.get(3) {
            assert!((val - 12.333333).abs() < 1e-5);
        } else {
            anyhow::bail!("Expected value at index 3");
        }
        if let Some(val) = s.get(4) {
            assert!((val - 13.333333).abs() < 1e-5);
        } else {
            anyhow::bail!("Expected value at index 4");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty, 5).is_err());

        let df_single = df!("close" => &[10.0])?;
        assert!(calculate(&df_single, 0).is_err()); // period 0 is invalid

        let res_single = calculate(&df_single, 2)?;
        assert!(res_single.f64()?.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100).map(|i| 100.0 + i as f64).collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 14)?;
        let s = result.f64()?;
        assert_eq!(s.len(), 100);
        assert!(s.get(0).is_none());
        assert!(s.get(12).is_none());
        assert!(s.get(13).is_some());
        assert!(s.get(99).is_some());
        Ok(())
    }
}
