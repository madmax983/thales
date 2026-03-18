//! Disparity Index
//!
//! Measures the relative position of the latest closing price to a chosen moving average.
//! A value greater than zero indicates that the price is above the moving average, while a
//! value less than zero indicates that the price is below the moving average.
//!
//! Formula: (Close - SMA) / SMA * 100

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::indicators::sma;

/// Calculate Disparity Index
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period for the SMA
///
/// # Returns
/// Series with Disparity Index values. The first `period - 1` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::disparity_index;
/// // let df = ...;
/// // let disparity = disparity_index::calculate(&df, 14)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.is_empty() {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Check for nulls in the "close" column
    let null_count = data.column("close")?.null_count();
    if null_count > 0 {
        anyhow::bail!("Close column contains null values");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let sma_series = sma::calculate(data, period).context("Failed to calculate SMA")?;
    let sma_f64 = sma_series.f64().context("SMA series must be numeric (f64)")?;

    let mut disparity_vals: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let hundred = Decimal::new(100, 0);

    for (c_opt, sma_opt) in close.into_iter().zip(sma_f64.into_iter()) {
        match (c_opt, sma_opt) {
            (Some(c), Some(s)) => {
                let c_dec_opt = Decimal::from_f64_retain(c);
                let s_dec_opt = Decimal::from_f64_retain(s);

                if let (Some(c_dec), Some(s_dec)) = (c_dec_opt, s_dec_opt) {
                    if s_dec.is_zero() {
                        disparity_vals.push(Some(0.0));
                    } else {
                        let disparity = (c_dec - s_dec) / s_dec * hundred;
                        disparity_vals.push(Some(disparity.to_f64().unwrap_or(0.0)));
                    }
                } else {
                    disparity_vals.push(None);
                }
            }
            _ => disparity_vals.push(None),
        }
    }

    Ok(Series::new("disparity_index", disparity_vals))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 10.0, 15.0, 20.0, 25.0]
        )?;

        // period = 3
        // SMA at i=2: (10 + 10 + 15)/3 = 11.6666...
        // Disparity = (15 - 11.6666) / 11.6666 * 100 = 3.3333 / 11.6666 * 100 = 28.5714...
        // SMA at i=3: (10 + 15 + 20)/3 = 15.0
        // Disparity = (20 - 15) / 15 * 100 = 33.3333...
        // SMA at i=4: (15 + 20 + 25)/3 = 20.0
        // Disparity = (25 - 20) / 20 * 100 = 25.0

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).unwrap();
        assert!((val2 - 28.5714).abs() < 0.001);

        let val3 = out.get(3).unwrap();
        assert!((val3 - 33.3333).abs() < 0.001);

        let val4 = out.get(4).unwrap();
        assert!((val4 - 25.0).abs() < 0.001);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert!(res_empty.unwrap_err().to_string().contains("Data cannot be empty"));

        // Single data point
        let df_single = df!("close" => &[10.0])?;
        let res_single = calculate(&df_single, 3)?;
        assert_eq!(res_single.len(), 1);
        assert!(res_single.f64()?.get(0).is_none());

        // Zero period
        let df_normal = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());

        // Null value in close column
        let df_null = df!("close" => &[Some(10.0), None, Some(12.0)])?;
        let res_null = calculate(&df_null, 2);
        assert!(res_null.is_err());
        assert!(res_null.unwrap_err().to_string().contains("null values"));

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..50).map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0).collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 14)?;

        assert_eq!(result.len(), 50);

        let out = result.f64()?;
        assert!(out.get(13).is_some());

        for i in 14..50 {
            let val = out.get(i).unwrap();
            // Disparity should be a relatively small percentage for a sine wave
            assert!(val > -50.0 && val < 50.0, "Disparity {} out of expected range at index {}", val, i);
        }

        Ok(())
    }
}
