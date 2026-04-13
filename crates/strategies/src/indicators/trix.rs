//! TRIX Indicator
//!
//! TRIX is the rate of change of a triple exponential moving average.
//! It is designed to filter out price movements that are insignificant
//! or shorter than the specified period.

use anyhow::{Context, Result};
use polars::prelude::*;

use super::ema;

/// Calculate the TRIX indicator
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - The period for the EMA
///
/// # Returns
/// Series containing the TRIX values.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Calculate EMA1
    let ema1 = ema::calculate(data, period).context("Failed to calculate EMA1")?;

    // Create df for EMA2
    let mut ema1_renamed = ema1.clone();
    ema1_renamed.rename("close");
    let ema1_df = DataFrame::new(vec![ema1_renamed])?;
    let ema2 = ema::calculate(&ema1_df, period).context("Failed to calculate EMA2")?;

    // Create df for EMA3
    let mut ema2_renamed = ema2.clone();
    ema2_renamed.rename("close");
    let ema2_df = DataFrame::new(vec![ema2_renamed])?;
    let ema3 = ema::calculate(&ema2_df, period).context("Failed to calculate EMA3")?;

    let ema3_arr = ema3.f64()?;

    let mut trix_vals: Vec<Option<f64>> = vec![None; data.height()];

    // TRIX = (EMA3 - EMA3_prev) / EMA3_prev * 100
    for (i, val) in trix_vals.iter_mut().enumerate().take(data.height()).skip(1) {
        if let (Some(curr), Some(prev)) = (ema3_arr.get(i), ema3_arr.get(i - 1)) {
            if prev != 0.0 {
                let trix = ((curr - prev) / prev) * 100.0;
                *val = Some(trix);
            }
        }
    }

    Ok(Series::new("trix", trix_vals))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_trix_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res = calculate(&df_empty, 2);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Data cannot be empty");

        let df_valid = df!("close" => &[10.0])?;
        let res = calculate(&df_valid, 0);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "Period must be greater than 0"
        );

        Ok(())
    }

    #[test]
    fn test_trix_calculation() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0]
        )?;

        let trix = calculate(&df, 2)?;
        let trix_arr = trix.f64()?;

        assert_eq!(trix_arr.len(), 9);

        Ok(())
    }
}
