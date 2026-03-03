//! ROC - Rate of Change
//!
//! Calculates the Rate of Change (ROC), a momentum oscillator that measures the percentage
//! change in price between the current price and the price a certain number of periods ago.
//!
//! Formula: ROC = ((Close - Close_n) / Close_n) * 100

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Rate of Change (ROC)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period for comparison
///
/// # Returns
/// Series with ROC values. The first `period` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let roc = strategies::indicators::roc::calculate(&df, 9)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let close_series = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .cast(&DataType::String)
        .context("Failed to cast close column to string for Decimal parsing")?;

    let close_str = close_series
        .str()
        .context("Failed to get string array from casted close column")?;

    let len = close_str.len();
    let mut roc_values: Vec<Option<String>> = vec![None; len];

    let hundred = Decimal::new(100, 0);

    for i in period..len {
        let curr_opt = close_str.get(i);
        let prev_opt = close_str.get(i - period);

        match (curr_opt, prev_opt) {
            (Some(curr_val), Some(prev_val)) => {
                if let (Ok(curr), Ok(prev)) = (
                    Decimal::from_str(curr_val),
                    Decimal::from_str(prev_val),
                ) {
                    if prev.is_zero() {
                        roc_values[i] = None;
                    } else {
                        let roc = ((curr - prev) / prev) * hundred;
                        roc_values[i] = Some(roc.to_string());
                    }
                } else {
                    roc_values[i] = None;
                }
            }
            _ => {
                roc_values[i] = None;
            }
        }
    }

    let out_series = Series::new("roc", roc_values);
    Ok(out_series)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;
    use std::str::FromStr;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &["10.0", "12.0", "15.0", "14.0"]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.str()?;

        assert_eq!(out.len(), 4);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = Decimal::from_str(out.get(2).unwrap())?;
        assert_eq!(val2, Decimal::new(50, 0)); // 50.0

        let val3 = Decimal::from_str(out.get(3).unwrap())?;
        // 16.666666666666666666666666667 (repeating)
        let expected = Decimal::from_str("16.666666666666666666666666670")?;
        assert_eq!(val3.round_dp(6), expected.round_dp(6));

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &["10.0", "11.0", "12.0"])?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.str()?;
        assert_eq!(out.len(), 3);
        assert!(out.get(0).is_none());
        assert!(out.get(2).is_none());

        let res_zero = calculate(&df_short, 0);
        assert!(res_zero.is_err());
        assert_eq!(res_zero.unwrap_err().to_string(), "Period must be greater than 0");

        let df_zero = df!("close" => &["0.0", "0.0", "10.0"])?;
        let res_div_zero = calculate(&df_zero, 2)?;
        let out_div_zero = res_div_zero.str()?;
        assert!(out_div_zero.get(2).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<String> = (0..50).map(|i| format!("{}.0", 100 + i)).collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 10);
        assert!(result.is_ok());
        let s = result?;
        assert_eq!(s.len(), 50);

        let out = s.str()?;
        assert!(out.get(9).is_none());
        assert!(out.get(10).is_some());

        let val10 = Decimal::from_str(out.get(10).unwrap())?;
        assert_eq!(val10, Decimal::new(10, 0)); // 10.0

        let val49 = Decimal::from_str(out.get(49).unwrap())?;
        let expected = Decimal::from_str("7.1942446043165467625899280600")?;
        assert_eq!(val49.round_dp(6), expected.round_dp(6));

        Ok(())
    }
}
