//! Rate of Change (ROC)
//!
//! A momentum oscillator that measures the percentage change in price between the current price and the price a certain number of periods ago.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Rate of Change (ROC)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with ROC values.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
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

    let len = close.len();
    let mut roc_values: Vec<Option<f64>> = vec![None; len];
    let hundred = Decimal::from(100);

    for i in period..len {
        let current_opt = close.get(i);
        let past_opt = close.get(i - period);

        if let (Some(current_val), Some(past_val)) = (current_opt, past_opt) {
            let current_dec = Decimal::from_f64_retain(current_val);
            let past_dec = Decimal::from_f64_retain(past_val);

            if let (Some(curr), Some(past)) = (current_dec, past_dec) {
                if past != Decimal::ZERO {
                    let roc = ((curr - past) / past) * hundred;
                    roc_values[i] = roc.to_f64();
                }
            }
        }
    }

    Ok(Series::new("roc", roc_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 12.0, 15.0, 14.0, 10.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 5);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert_eq!(out.get(2), Some(50.0));

        // (14-12)/12 * 100 = 16.666666...
        assert!((out.get(3).unwrap() - 16.666666666666668).abs() < 1e-6);

        // (10-15)/15 * 100 = -33.333333...
        assert!((out.get(4).unwrap() - -33.333333333333336).abs() < 1e-6);

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
        let out = res_short.f64()?;
        assert!(out.get(0).is_none());

        let df_zero = df!("close" => &[10.0, 12.0])?;
        let res_zero = calculate(&df_zero, 0);
        assert!(res_zero.is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[100.0, 101.0, 102.0, 101.5, 99.0, 98.0, 105.0]
        )?;

        let s = calculate(&df, 3)?;
        let out = s.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());
        assert_eq!(out.get(3), Some(1.5));
        assert!((out.get(4).unwrap() - -1.9801980198019802).abs() < 1e-6);

        Ok(())
    }
}
