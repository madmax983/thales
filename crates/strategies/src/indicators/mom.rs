//! Momentum (MOM)
//!
//! Measures the rate of change of a security's price.
//! Momentum compares the current price with the previous price from a number of periods ago.
//! Positive values indicate an upward trend; negative values indicate a downward trend.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Momentum (MOM)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period (typically 10 or 14)
///
/// # Returns
/// Series with MOM values. The first `period` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::mom;
///
/// let df = df!(
///     "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
/// ).unwrap_or_default();
/// let result = mom::calculate(&df, 2).unwrap_or_default();
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

    let close = close_s.f64().context("Close column must be numeric")?;

    // Pre-extract into contiguous Vec<Option<Decimal>>
    let decimal_close: Vec<Option<Decimal>> = close
        .into_iter()
        .map(|opt_val| {
            if let Some(val) = opt_val {
                Decimal::from_f64_retain(val)
            } else {
                None
            }
        })
        .collect();

    let mut mom_values: Vec<Option<f64>> = vec![None; decimal_close.len()];

    for (i, mom_out) in mom_values
        .iter_mut()
        .enumerate()
        .take(decimal_close.len())
        .skip(period)
    {
        if let (Some(curr), Some(prev)) = (decimal_close[i], decimal_close[i - period]) {
            let mom = curr - prev;
            *mom_out = mom.to_f64();
        }
    }

    Ok(Series::new("mom", mom_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_mom_calculation() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 12.0, 15.0, 14.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 4);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).unwrap_or(0.0);
        assert!((val2 - 5.0).abs() < 1e-10);

        let val3 = out.get(3).unwrap_or(0.0);
        assert!((val3 - 2.0).abs() < 1e-10);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 10);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period 0
        let df_normal = df!("close" => &[10.0, 11.0])?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());
        assert_eq!(
            res_zero.unwrap_err().to_string(),
            "Period must be greater than 0"
        );

        // Period > Data length
        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.f64()?;
        assert_eq!(out.len(), 2);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64 * 0.5)).collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 14)?;

        assert_eq!(result.len(), 100);
        let out = result.f64()?;

        assert!(out.get(13).is_none());
        assert!(out.get(14).is_some());
        assert!(out.get(99).is_some());

        // For a constant linear increase of 0.5 per period, MOM(14) should be 14 * 0.5 = 7.0
        let val99 = out.get(99).unwrap_or(0.0);
        assert!((val99 - 7.0).abs() < 1e-10);

        Ok(())
    }
}
