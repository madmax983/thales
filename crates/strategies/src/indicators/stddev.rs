//! Standard Deviation (StdDev)
//!
//! Calculates the Standard Deviation of price data over a specified lookback period.
//! Standard Deviation is a statistical measure of market volatility.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Standard Deviation
///
/// # Arguments
/// * `data` - DataFrame with a numeric column
/// * `column_name` - Name of the column to calculate Standard Deviation for (e.g., "close")
/// * `period` - Lookback period (must be > 0)
///
/// # Returns
/// Series with Standard Deviation values. The first `period - 1` values will be null.
///
/// # Example
/// ```rust
/// use strategies::indicators::stddev;
/// use polars::prelude::*;
/// // let df = ...;
/// // let stddev = stddev::calculate(&df, "close", 20)?;
/// ```
pub fn calculate(data: &DataFrame, column_name: &str, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let values = data
        .column(column_name)
        .context(format!("DataFrame must contain '{}' column", column_name))?
        .f64()
        .context(format!("{} column must be numeric (f64)", column_name))?;

    let mut stddev_band: Vec<Option<f64>> = Vec::with_capacity(values.len());

    let mut window: VecDeque<Decimal> = VecDeque::with_capacity(period);
    let mut sum_x = Decimal::ZERO;
    let mut sum_x2 = Decimal::ZERO;

    let period_dec =
        Decimal::from_usize(period).context("Invalid period for Decimal conversion")?;

    for i in 0..values.len() {
        let val_opt = values.get(i);

        match val_opt {
            Some(val) => {
                if let Some(d) = Decimal::from_f64_retain(val) {
                    sum_x += d;
                    sum_x2 += d * d;
                    window.push_back(d);

                    if window.len() > period {
                        if let Some(old) = window.pop_front() {
                            sum_x -= old;
                            sum_x2 -= old * old;
                        }
                    }

                    if window.len() == period {
                        let mean = sum_x / period_dec;
                        let variance_term1 = sum_x2 / period_dec;
                        let variance_term2 = mean * mean;
                        let variance = variance_term1 - variance_term2;

                        let variance = if variance < Decimal::ZERO {
                            Decimal::ZERO
                        } else {
                            variance
                        };

                        let std_dev = variance.sqrt().unwrap_or(Decimal::ZERO);
                        stddev_band.push(std_dev.to_f64());
                    } else {
                        stddev_band.push(None);
                    }
                } else {
                    window.clear();
                    sum_x = Decimal::ZERO;
                    sum_x2 = Decimal::ZERO;
                    stddev_band.push(None);
                }
            }
            None => {
                window.clear();
                sum_x = Decimal::ZERO;
                sum_x2 = Decimal::ZERO;
                stddev_band.push(None);
            }
        }
    }

    Ok(Series::new("stddev", stddev_band))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_stddev_known_values() -> Result<()> {
        // Simple dataset: [10, 10, 10, 10, 10]
        // StdDev(3) = 0
        let df_stable = df!(
            "close" => &[10.0, 10.0, 10.0, 10.0, 10.0]
        )?;
        let result = calculate(&df_stable, "close", 3)?;
        let std_vals = result.f64()?;
        assert!(std_vals.get(0).is_none());
        assert!(std_vals.get(1).is_none());
        assert_eq!(std_vals.get(2), Some(0.0));
        assert_eq!(std_vals.get(3), Some(0.0));
        assert_eq!(std_vals.get(4), Some(0.0));

        // Dataset with variance: [10, 12, 14, 16, 18]
        // Index 2: [10, 12, 14]. Variance = ((10-12)^2 + (12-12)^2 + (14-12)^2) / 3 = 8/3 = 2.666...
        // StdDev = sqrt(2.666...) ≈ 1.63299
        let df_var = df!(
            "close" => &[10.0, 12.0, 14.0, 16.0, 18.0]
        )?;
        let result_v = calculate(&df_var, "close", 3)?;
        let u = result_v.f64()?;

        let eps = 0.0001;
        let std_val = u.get(2).unwrap();

        assert!(
            (std_val - 1.63299).abs() < eps,
            "StdDev mismatch: {} vs 1.63299",
            std_val
        );

        Ok(())
    }

    #[test]
    fn test_stddev_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, "close", 5);
        assert!(res_empty.is_err());

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res = calculate(&df_short, "close", 5)?;
        assert!(res.f64()?.get(0).is_none());
        assert!(res.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_stddev_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64)).collect();
        let df = df!("close" => values)?;

        let res = calculate(&df, "close", 14)?;
        assert_eq!(res.len(), 100);

        for i in 0..13 {
            assert!(res.f64()?.get(i).is_none());
        }
        assert!(res.f64()?.get(13).is_some());

        Ok(())
    }
}
