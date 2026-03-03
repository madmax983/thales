//! Rate of Change (ROC) - A momentum indicator that measures the percentage change in price between the current price and the price a certain number of periods ago.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;

/// Calculate Rate of Change (ROC)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with ROC values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::roc;
///
/// let df = DataFrame::default(); // Load data
/// // let result = roc::calculate(&df, 14)?;
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

    let mut roc_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let hundred = Decimal::new(100, 0);

    for i in 0..close.len() {
        if i < period {
            roc_values.push(None);
            continue;
        }

        let current_val_opt = close.get(i);
        let past_val_opt = close.get(i - period);

        match (current_val_opt, past_val_opt) {
            (Some(current_val), Some(past_val)) => {
                if let (Some(current_d), Some(past_d)) = (
                    Decimal::from_f64_retain(current_val),
                    Decimal::from_f64_retain(past_val),
                ) {
                    if past_d.is_zero() {
                        roc_values.push(None);
                    } else {
                        let roc = ((current_d - past_d) / past_d) * hundred;
                        roc_values.push(roc.to_f64());
                    }
                } else {
                    roc_values.push(None);
                }
            }
            _ => roc_values.push(None),
        }
    }

    let s = Series::new("roc", roc_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[100.0, 105.0, 110.0, 108.0, 115.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        // Expected:
        // i=0: None
        // i=1: None
        // i=2: (110 - 100) / 100 * 100 = 10.0
        // i=3: (108 - 105) / 105 * 100 = 2.857...
        // i=4: (115 - 110) / 110 * 100 = 4.545...

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert_eq!(out.get(2), Some(10.0));

        let val3 = out.get(3).unwrap();
        assert!((val3 - 2.857142857142857).abs() < 1e-10);

        let val4 = out.get(4).unwrap();
        assert!((val4 - 4.545454545454545).abs() < 1e-10);

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

        // Zero in past value
        let df_zero = df!("close" => &[0.0, 10.0])?;
        let res_zero = calculate(&df_zero, 1)?;
        assert_eq!(res_zero.len(), 2);
        assert!(res_zero.f64()?.get(0).is_none());
        assert!(res_zero.f64()?.get(1).is_none()); // Division by zero prevented

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[100.0, 102.0, 101.0, 103.0, 102.0]
        )?;

        // Period 1
        let s = calculate(&df, 1)?;
        let out = s.f64()?;

        assert!(out.get(0).is_none());
        assert_eq!(out.get(1), Some(2.0)); // (102-100)/100 * 100

        let val2 = out.get(2).unwrap();
        assert!((val2 - (-0.980392156862745)).abs() < 1e-10); // (101-102)/102 * 100

        Ok(())
    }
}
