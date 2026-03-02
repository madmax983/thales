use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Z-Score
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period for SMA and Standard Deviation
///
/// # Returns
/// Series with Z-Score values.
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

    let mut zscore_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut window: VecDeque<Decimal> = VecDeque::with_capacity(period);

    let mut sum_x = Decimal::ZERO;
    let mut sum_x2 = Decimal::ZERO; // Sum of x^2

    let period_dec = Decimal::from_usize(period).context("Invalid period for Decimal conversion")?;

    for i in 0..close.len() {
        let val_opt = close.get(i);

        match val_opt {
            Some(val) => {
                if let Some(d) = Decimal::from_f64_retain(val) {
                    // Update rolling sums
                    sum_x += d;
                    sum_x2 += d * d;
                    window.push_back(d);

                    // Maintain window size
                    if window.len() > period {
                        if let Some(old) = window.pop_front() {
                            sum_x -= old;
                            sum_x2 -= old * old;
                        }
                    }

                    if window.len() == period {
                        // Calculate stats
                        let mean = sum_x / period_dec;

                        // Variance = (Sum(x^2) / N) - (Mean^2)
                        let variance_term1 = sum_x2 / period_dec;
                        let variance_term2 = mean * mean;
                        let variance = variance_term1 - variance_term2;

                        // Clamp variance to 0 if slightly negative due to precision
                        let variance = if variance < Decimal::ZERO {
                            Decimal::ZERO
                        } else {
                            variance
                        };

                        let std_dev = variance.sqrt().unwrap_or(Decimal::ZERO);

                        if std_dev.is_zero() {
                            zscore_values.push(Some(0.0));
                        } else {
                            let z_score = (d - mean) / std_dev;
                            zscore_values.push(z_score.to_f64());
                        }
                    } else {
                        // Not enough data yet
                        zscore_values.push(None);
                    }
                } else {
                    // Invalid float (e.g. NaN)
                    // Reset
                    window.clear();
                    sum_x = Decimal::ZERO;
                    sum_x2 = Decimal::ZERO;
                    zscore_values.push(None);
                }
            }
            None => {
                // Missing data
                // Reset
                window.clear();
                sum_x = Decimal::ZERO;
                sum_x2 = Decimal::ZERO;
                zscore_values.push(None);
            }
        }
    }

    let s = Series::new("zscore", zscore_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df_stable = df!(
            "close" => &[10.0, 10.0, 10.0, 10.0, 10.0]
        )?;
        let result = calculate(&df_stable, 3)?;
        let z_vals = result.f64()?;

        assert!(z_vals.get(0).is_none());
        assert!(z_vals.get(1).is_none());
        assert_eq!(z_vals.get(2), Some(0.0));
        assert_eq!(z_vals.get(3), Some(0.0));

        let df_var = df!(
            "close" => &[10.0, 12.0, 14.0, 16.0, 18.0]
        )?;
        let result_v = calculate(&df_var, 3)?;
        let z_v_vals = result_v.f64()?;

        // Index 2: [10, 12, 14]. Mean = 12. StdDev = sqrt(2.666...) = 1.63299
        // d = 14
        // Z = (14 - 12) / 1.63299 = 2 / 1.63299 = 1.2247448...
        let val = z_v_vals.get(2).unwrap();
        assert!((val - 1.2247448).abs() < 0.0001);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0, 11.0])?;
        let result = calculate(&df_short, 5)?;
        assert!(result.f64()?.get(0).is_none());
        assert!(result.f64()?.get(1).is_none());

        Ok(())
    }
}
