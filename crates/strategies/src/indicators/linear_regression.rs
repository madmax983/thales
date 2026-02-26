use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Linear Regression Slope
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with Slope values.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period < 2 {
        anyhow::bail!("Period must be at least 2");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let mut slope_values: Vec<Option<f64>> = vec![None; close.len()];

    let n = Decimal::from_usize(period).context("Invalid period")?;
    let n_f64 = n.to_f64().unwrap();

    // Constant terms involving x (0..N-1)
    // Sum X = N*(N-1)/2
    let sum_x = (n_f64 * (n_f64 - 1.0)) / 2.0;
    // Sum X^2 = (N-1)*N*(2N-1)/6
    let sum_x_sq = ((n_f64 - 1.0) * n_f64 * (2.0 * n_f64 - 1.0)) / 6.0;

    let denom = n_f64 * sum_x_sq - sum_x * sum_x;

    if denom.abs() < 1e-10 {
        // Should not happen for N >= 2 as variance of 0..N-1 is non-zero
        anyhow::bail!("Denominator is zero, invalid period?");
    }

    let mut window: VecDeque<f64> = VecDeque::with_capacity(period);
    let mut sum_y = 0.0;
    let mut sum_xy = 0.0;

    for (val_opt, slope_slot) in close.into_iter().zip(slope_values.iter_mut()) {
        if let Some(val) = val_opt {
            if val.is_nan() {
                window.clear();
                sum_y = 0.0;
                sum_xy = 0.0;
                continue;
            }

            // Update sum_y
            sum_y += val;
            window.push_back(val);

            if window.len() <= period {
                // Filling phase
                // x is just the index in the window: window.len() - 1
                sum_xy += (window.len() as f64 - 1.0) * val;
            } else {
                // Sliding phase
                let old = window.pop_front().unwrap();

                // sum_y currently includes 'old' + 'intermediates' + 'val'
                // sum_y_prev (sum of previous window) = sum_y - val
                let sum_y_prev = sum_y - val;

                // Update sum_xy
                // S_new = S_old - Sum_Y_prev + old + (N-1)*val
                sum_xy = sum_xy - sum_y_prev + old + (n_f64 - 1.0) * val;

                // Update sum_y to remove old
                sum_y -= old;
            }

            if window.len() == period {
                let num = n_f64 * sum_xy - sum_x * sum_y;
                let slope = num / denom;
                *slope_slot = Some(slope);
            }
        } else {
            window.clear();
            sum_y = 0.0;
            sum_xy = 0.0;
        }
    }

    Ok(Series::new("slope", slope_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        // Data: [1, 2, 3, 4, 5]
        // Slope should be 1.0 everywhere for any period
        let df = df!(
            "close" => &[1.0, 2.0, 3.0, 4.0, 5.0]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 5);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!((out.get(2).unwrap() - 1.0).abs() < 1e-6);
        assert!((out.get(3).unwrap() - 1.0).abs() < 1e-6);
        assert!((out.get(4).unwrap() - 1.0).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn test_negative_slope() -> Result<()> {
        // Data: [5, 4, 3, 2, 1]
        // Slope -1.0
        let df = df!(
            "close" => &[5.0, 4.0, 3.0, 2.0, 1.0]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!((out.get(2).unwrap() - -1.0).abs() < 1e-6);
        Ok(())
    }

    #[test]
    fn test_constant_slope() -> Result<()> {
        // Data: [5, 5, 5]
        // Slope 0.0
        let df = df!(
            "close" => &[5.0, 5.0, 5.0]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!((out.get(2).unwrap() - 0.0).abs() < 1e-6);
        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());

        // Period > Data length
        let df_short = df!("close" => &[10.0])?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.f64()?;
        assert!(out.get(0).is_none());

        Ok(())
    }
}
