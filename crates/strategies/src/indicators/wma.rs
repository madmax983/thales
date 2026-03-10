//! Weighted Moving Average (WMA)
//!
//! A weighted moving average assigns a heavier weighting to more current data points since they are more relevant than data points in the distant past.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

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
/// // let df = // ... load data
/// // let result = wma::calculate(&df, 14)?;
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

    let mut wma_values: Vec<Option<f64>> = Vec::with_capacity(close.len());

    // Total sum of weights for a given period N is N*(N+1)/2
    let sum_weights = Decimal::from_usize((period * (period + 1)) / 2)
        .context("Failed to create sum_weights Decimal")?;

    for i in 0..close.len() {
        if i < period - 1 {
            wma_values.push(None);
            continue;
        }

        let mut sum = Decimal::ZERO;
        let mut valid = true;

        for j in 0..period {
            let idx = i + 1 + j - period;
            let weight = Decimal::from_usize(j + 1).context("Failed to create weight Decimal")?;

            if let Some(val) = close.get(idx) {
                if let Some(d) = Decimal::from_f64_retain(val) {
                    sum += d * weight;
                } else {
                    valid = false;
                    break;
                }
            } else {
                valid = false;
                break;
            }
        }

        if valid {
            let avg = sum / sum_weights;
            wma_values.push(Some(avg.to_f64().context("Failed to convert Decimal to f64")?));
        } else {
            wma_values.push(None);
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
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
        )?;

        // Period 3
        // sum of weights = 3 + 2 + 1 = 6
        // index 0, 1: null
        // index 2: (12*3 + 11*2 + 10*1) / 6 = (36 + 22 + 10) / 6 = 68 / 6 = 11.333333
        // index 3: (13*3 + 12*2 + 11*1) / 6 = (39 + 24 + 11) / 6 = 74 / 6 = 12.333333
        // index 4: (14*3 + 13*2 + 12*1) / 6 = (42 + 26 + 12) / 6 = 80 / 6 = 13.333333

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).unwrap();
        assert!((val2 - 11.333333).abs() < 1e-5);

        let val3 = out.get(3).unwrap();
        assert!((val3 - 12.333333).abs() < 1e-5);

        let val4 = out.get(4).unwrap();
        assert!((val4 - 13.333333).abs() < 1e-5);

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
        // sum of weights = 2 + 1 = 3
        // index 0: null
        // index 1: (102*2 + 100*1)/3 = (204 + 100)/3 = 304/3 = 101.333333
        // index 2: (101*2 + 102*1)/3 = (202 + 102)/3 = 304/3 = 101.333333
        // index 3: (103*2 + 101*1)/3 = (206 + 101)/3 = 307/3 = 102.333333
        // index 4: (102*2 + 103*1)/3 = (204 + 103)/3 = 307/3 = 102.333333

        let s = calculate(&df, 2)?;
        let out = s.f64()?;

        assert!(out.get(0).is_none());

        let val1 = out.get(1).unwrap();
        assert!((val1 - 101.333333).abs() < 1e-5);

        let val2 = out.get(2).unwrap();
        assert!((val2 - 101.333333).abs() < 1e-5);

        let val3 = out.get(3).unwrap();
        assert!((val3 - 102.333333).abs() < 1e-5);

        let val4 = out.get(4).unwrap();
        assert!((val4 - 102.333333).abs() < 1e-5);

        Ok(())
    }
}
