//! Mass Index
//!
//! Calculates the Mass Index, an indicator that uses the high-low range to identify
//! trend reversals based on range expansions.
//!
//! The calculation follows these steps:
//! 1. Single EMA = EMA of (High - Low)
//! 2. Double EMA = EMA of (Single EMA)
//! 3. EMA Ratio = Single EMA / Double EMA
//! 4. Mass Index = Sum of EMA Ratio over a given period
//!
//! # Returns
//! Series of `f64` values representing the Mass Index.
//!
//! # Example
//!
//! ```rust
//! use polars::prelude::*;
//! use strategies::indicators::mass_index;
//!
//! // Assuming df is a DataFrame containing "high" and "low" columns
//! let df = df!(
//!     "high" => &[10.0, 11.0, 12.0, 13.0, 14.0],
//!     "low" =>  &[ 9.0,  9.5, 10.0, 11.0, 12.0]
//! ).unwrap();
//!
//! // Calculates Mass Index using EMA period 9 and Sum period 25
//! // (Here using 2 and 3 for short data)
//! let mi = mass_index::calculate(&df, 2, 3).unwrap();
//! ```

use anyhow::{Context, Result};
use polars::prelude::*;

use crate::indicators::ema;

/// Calculate the Mass Index
///
/// # Arguments
/// * `data` - DataFrame with "high" and "low" columns
/// * `ema_period` - Lookback period for the Single and Double EMAs (typically 9)
/// * `sum_period` - Lookback period for the sum of the EMA Ratios (typically 25)
///
/// # Returns
/// Series with Mass Index values. First few values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let mi = strategies::indicators::mass_index::calculate(&df, 9, 25)?;
/// ```
pub fn calculate(data: &DataFrame, ema_period: usize, sum_period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if ema_period == 0 || sum_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

    let high = data.column("high").context("DataFrame must contain 'high' column")?;
    let low = data.column("low").context("DataFrame must contain 'low' column")?;

    // Calculate High - Low (Range)
    // We do this using Polars Series arithmetic natively
    let mut range_series = (high - low)?;
    range_series.rename("close"); // rename so ema can find it

    let range_df = DataFrame::new(vec![range_series])?;

    // 1. Single EMA of the Range
    let mut single_ema = ema::calculate(&range_df, ema_period)?;
    single_ema.rename("close");
    let single_ema_df = DataFrame::new(vec![single_ema.clone()])?;

    // 2. Double EMA of the Single EMA (applying EMA again)
    let double_ema = ema::calculate(&single_ema_df, ema_period)?;

    // 3. EMA Ratio = Single EMA / Double EMA
    // We can do this with vector arithmetic natively in Polars
    let ratio_series = (&single_ema / &double_ema)?;
    let ratio_df = DataFrame::new(vec![ratio_series.clone()])?;

    // 4. Sum of the EMA Ratio over sum_period
    // Use Polars native rolling window
    let mut lazy_df = ratio_df.lazy();
    lazy_df = lazy_df.select([
        col("close").rolling_sum(RollingOptionsFixedWindow {
            window_size: sum_period,
            min_periods: sum_period,
            weights: None,
            center: false,
            fn_params: None,
        })
    ]);

    let final_df = lazy_df.collect()?;
    let mass_index_series = final_df.column("close")?;

    let mut renamed_series = mass_index_series.clone();
    renamed_series.rename("mass_index");

    Ok(renamed_series)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_mass_index_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "high" => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "low" =>  &[ 8.0,  8.0,  8.0,  8.0,  8.0,  8.0,  8.0]
        )?;

        let result = calculate(&df, 2, 3)?;
        let out = result.f64()?;

        let v4 = out.get(4).context("Expected value at index 4")?;
        if (v4 - 3.0).abs() >= 0.001 {
            anyhow::bail!("Expected 3.0 at index 4, got {}", v4);
        }

        let v5 = out.get(5).context("Expected value at index 5")?;
        if (v5 - 3.0).abs() >= 0.001 {
            anyhow::bail!("Expected 3.0 at index 5, got {}", v5);
        }

        let v6 = out.get(6).context("Expected value at index 6")?;
        if (v6 - 3.0).abs() >= 0.001 {
            anyhow::bail!("Expected 3.0 at index 6, got {}", v6);
        }

        Ok(())
    }

    #[test]
    fn test_mass_index_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 9, 25);
        if res_empty.is_ok() {
            anyhow::bail!("Expected error for empty data");
        }
        if res_empty.unwrap_err().to_string() != "Data cannot be empty" {
            anyhow::bail!("Unexpected error message for empty data");
        }

        let df_missing_cols = df!("close" => &[10.0, 11.0])?;
        let res_missing = calculate(&df_missing_cols, 9, 25);
        if res_missing.is_ok() {
            anyhow::bail!("Expected error for missing columns");
        }
        if !res_missing.unwrap_err().to_string().contains("must contain 'high' column") {
            anyhow::bail!("Unexpected error message for missing columns");
        }

        let df_valid = df!("high" => &[10.0], "low" => &[9.0])?;
        let res_zero_period = calculate(&df_valid, 0, 25);
        if res_zero_period.is_ok() {
            anyhow::bail!("Expected error for zero period");
        }
        if res_zero_period.unwrap_err().to_string() != "Periods must be greater than 0" {
            anyhow::bail!("Unexpected error message for zero period");
        }

        Ok(())
    }

    #[test]
    fn test_mass_index_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[
                102.5, 103.5, 104.5, 105.5, 106.5, 107.5, 108.5, 109.5, 110.5, 111.5, 112.5
            ],
            "high" => &[
                105.0, 106.0, 107.0, 108.0, 109.0, 110.0, 111.0, 112.0, 113.0, 114.0, 115.0
            ],
            "low" => &[
                100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0, 108.0, 109.0, 110.0
            ]
        )?;

        let result = calculate(&df, 3, 4)?;
        if result.len() != 11 {
            anyhow::bail!("Expected result length 11, got {}", result.len());
        }

        let out = result.f64()?;
        let final_val = out.get(10).context("Expected value at index 10")?;
        if final_val <= 0.0 {
            anyhow::bail!("Expected positive value, got {}", final_val);
        }

        Ok(())
    }
}
