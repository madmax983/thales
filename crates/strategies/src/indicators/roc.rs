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
/// Series with ROC values. The first `period` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::roc;
/// let df = df!("close" => &[10.0, 12.0, 15.0]).unwrap();
/// let result = roc::calculate(&df, 1).unwrap();
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

    let len = close.len();
    let mut roc_values: Vec<Option<f64>> = vec![None; len];

    if len <= period {
        return Ok(Series::new("roc", roc_values));
    }

    let hundred = Decimal::from(100);

    for (i, val) in roc_values.iter_mut().enumerate().skip(period) {
        let current_val_opt = close.get(i);
        let past_val_opt = close.get(i - period);

        match (current_val_opt, past_val_opt) {
            (Some(current), Some(past)) => {
                let current_dec = Decimal::from_f64_retain(current);
                let past_dec = Decimal::from_f64_retain(past);

                if let (Some(c), Some(p)) = (current_dec, past_dec) {
                    if p.is_zero() {
                        *val = None;
                    } else {
                        // ROC = ((Close - Close_n_periods_ago) / Close_n_periods_ago) * 100
                        let diff = c - p;
                        let roc = (diff.checked_div(p).unwrap_or(Decimal::ZERO)) * hundred;
                        *val = Some(roc.to_f64().unwrap_or(f64::NAN));
                    }
                } else {
                    *val = None;
                }
            }
            _ => {
                *val = None;
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

        // i=2, period=2: (15 - 10) / 10 * 100 = 50.0
        assert_eq!(out.get(2), Some(50.0));

        // i=3, period=2: (14 - 12) / 12 * 100 = 16.666...
        let val3 = out.get(3).unwrap();
        assert!((val3 - 16.666).abs() < 1e-2);

        // i=4, period=2: (10 - 15) / 15 * 100 = -33.333...
        let val4 = out.get(4).unwrap();
        assert!((val4 - -33.333).abs() < 1e-2);

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

        // Zero previous value
        let df_zero = df!("close" => &[0.0, 10.0])?;
        let res_zero = calculate(&df_zero, 1)?;
        assert!(res_zero.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[100.0, 102.0, 101.0, 103.0, 102.0]
        )?;

        let s = calculate(&df, 1)?;
        let out = s.f64()?;

        assert!(out.get(0).is_none());
        assert_eq!(out.get(1), Some(2.0)); // (102-100)/100 * 100 = 2.0
        assert!((out.get(2).unwrap() - -0.9803).abs() < 1e-3); // (101-102)/102 * 100 = -0.9803
        assert!((out.get(3).unwrap() - 1.9801).abs() < 1e-3); // (103-101)/101 * 100 = 1.9801
        assert!((out.get(4).unwrap() - -0.9708).abs() < 1e-3); // (102-103)/103 * 100 = -0.9708

        Ok(())
    }
}
