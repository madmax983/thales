//! Williams %R - Williams Percent Range
//!
//! Calculates the Williams %R, a momentum indicator that moves between 0 and -100
//! and measures overbought and oversold levels. The Williams %R compares a stock's
//! closing price to the high-low range over a specific period.
//!
//! Traditionally, Williams %R is considered overbought when above -20 and oversold when below -80.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Williams %R
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns
/// * `period` - Lookback period (standard is 14)
///
/// # Returns
/// Series with Williams %R values. The first `period - 1` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let will_r = strategies::indicators::williams_r::calculate(&df, 14)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Check if columns exist and are numeric
    let high = data
        .column("high")
        .context("DataFrame must contain 'high' column")?
        .f64()
        .context("High column must be numeric (f64)")?;

    let low = data
        .column("low")
        .context("DataFrame must contain 'low' column")?
        .f64()
        .context("Low column must be numeric (f64)")?;

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let close_len = data.height();

    // We strictly return a Float64 ChunkedArray / Series to integrate with the rest of the codebase (e.g. indicators like RSI)
    // but we use Decimal internally for the mathematical calculations.
    let mut williams_r_values: Vec<Option<f64>> = vec![None; close_len];

    if close_len < period {
        return Ok(Series::new("williams_r", williams_r_values));
    }

    let rolling_opts = RollingOptionsFixedWindow {
        window_size: period,
        min_periods: period,
        weights: None,
        center: false,
        fn_params: None,
    };

    let lf = data.clone().lazy();

    let computed = lf.with_columns(vec![
        col("high").rolling_max(rolling_opts.clone().into()).alias("hh"),
        col("low").rolling_min(rolling_opts.into()).alias("ll"),
    ]).collect()?;

    let hh_series = computed.column("hh")?.f64()?;
    let ll_series = computed.column("ll")?.f64()?;

    let minus_hundred = Decimal::new(-100, 0);

    for i in (period - 1)..close_len {
        let current_close = close.get(i);
        let current_hh = hh_series.get(i);
        let current_ll = ll_series.get(i);

        if let (Some(close_val), Some(hh), Some(ll)) = (current_close, current_hh, current_ll) {
            if !hh.is_nan() && !ll.is_nan() && !close_val.is_nan() {
                // Using rust_decimal for ALL financial calculation steps per requirements
                let hh_dec = Decimal::from_f64_retain(hh).unwrap_or(Decimal::ZERO);
                let ll_dec = Decimal::from_f64_retain(ll).unwrap_or(Decimal::ZERO);
                let close_dec = Decimal::from_f64_retain(close_val).unwrap_or(Decimal::ZERO);

                let range = hh_dec - ll_dec;

                if range.is_zero() {
                    // If highest high == lowest low, default to -50
                    williams_r_values[i] = Some(-50.0);
                } else {
                    // %R = (Highest High - Close) / (Highest High - Lowest Low) * -100
                    let numerator = hh_dec - close_dec;
                    let will_r = (numerator / range) * minus_hundred;
                    williams_r_values[i] = Some(will_r.to_f64().unwrap_or(f64::NAN));
                }
            }
        }
    }

    Ok(Series::new("williams_r", williams_r_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" => &[12.0, 14.0, 13.0, 15.0, 14.0],
            "low" => &[10.0, 11.0, 9.0, 12.0, 13.0],
            "close" => &[11.0, 13.0, 12.0, 14.0, 13.5]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 5);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        if let Some(val2) = out.get(2) {
            assert!((val2 - -40.0).abs() < 1e-4, "Expected -40.0, got {}", val2);
        } else {
            anyhow::bail!("Value at index 2 is None");
        }

        if let Some(val3) = out.get(3) {
            assert!((val3 - -16.666666).abs() < 1e-4, "Expected ~-16.67, got {}", val3);
        } else {
            anyhow::bail!("Value at index 3 is None");
        }

        if let Some(val4) = out.get(4) {
            assert!((val4 - -25.0).abs() < 1e-4, "Expected -25.0, got {}", val4);
        } else {
            anyhow::bail!("Value at index 4 is None");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());

        let df_short = df!(
            "high" => &[10.0, 11.0, 12.0],
            "low" => &[9.0, 10.0, 11.0],
            "close" => &[9.5, 10.5, 11.5]
        )?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.f64()?;
        assert_eq!(out.len(), 3);
        assert!(out.get(0).is_none());

        let df_normal = df!(
            "high" => &[10.0, 11.0],
            "low" => &[9.0, 10.0],
            "close" => &[9.5, 10.5]
        )?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());

        let df_flat = df!(
            "high" => &[10.0, 10.0, 10.0, 10.0],
            "low" => &[10.0, 10.0, 10.0, 10.0],
            "close" => &[10.0, 10.0, 10.0, 10.0]
        )?;
        let res_flat = calculate(&df_flat, 2)?;
        let out_flat = res_flat.f64()?;
        assert_eq!(out_flat.get(2), Some(-50.0));

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let highs: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0 + 2.0).collect();
        let lows: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0 - 2.0).collect();
        let closes: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0).collect();

        let df = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let result = calculate(&df, 14);
        assert!(result.is_ok());
        let s = result?;
        assert_eq!(s.len(), 100);
        assert!(s.f64()?.get(12).is_none());

        Ok(())
    }
}
