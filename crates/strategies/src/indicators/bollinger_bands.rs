//! Bollinger Bands - A volatility indicator consisting of a simple moving average (SMA) and two standard deviation bands.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Bollinger Bands
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period for SMA and Standard Deviation
/// * `std_dev_multiplier` - Number of standard deviations for the bands (typically 2.0)
///
/// # Returns
/// Tuple of (Lower Band, Middle Band, Upper Band) Series.
///
/// # Example
/// ```rust
/// use strategies::indicators::bollinger_bands;
/// use polars::prelude::*;
///
/// // Assuming df is a DataFrame with a "close" column
/// let period = 20;
/// let k = 2.0;
/// // let (lower, middle, upper) = bollinger_bands::calculate(&df, period, k)?;
/// ```
pub fn calculate(
    data: &DataFrame,
    period: usize,
    std_dev_multiplier: f64,
) -> Result<(Series, Series, Series)> {
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

    let mut lower_band: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut middle_band: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut upper_band: Vec<Option<f64>> = Vec::with_capacity(close.len());

    let mut window: VecDeque<Decimal> = VecDeque::with_capacity(period);
    let mut sum_x = Decimal::ZERO;
    let mut sum_x2 = Decimal::ZERO; // Sum of x^2

    let period_dec = Decimal::from_usize(period).context("Invalid period for Decimal conversion")?;
    let k_dec = Decimal::from_f64_retain(std_dev_multiplier).unwrap_or(Decimal::ZERO);

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
                        // Note: floating point precision might make this slightly negative close to 0
                        let variance_term1 = sum_x2 / period_dec;
                        let variance_term2 = mean * mean;
                        let variance = variance_term1 - variance_term2;

                        // Clamp variance to 0 if slightly negative due to precision
                        let variance = if variance < Decimal::ZERO {
                            Decimal::ZERO
                        } else {
                            variance
                        };

                        // sqrt returns None if negative, but we clamped it.
                        let std_dev = variance.sqrt().unwrap_or(Decimal::ZERO);

                        let upper = mean + (k_dec * std_dev);
                        let lower = mean - (k_dec * std_dev);

                        middle_band.push(mean.to_f64());
                        upper_band.push(upper.to_f64());
                        lower_band.push(lower.to_f64());
                    } else {
                        // Not enough data yet
                        middle_band.push(None);
                        upper_band.push(None);
                        lower_band.push(None);
                    }
                } else {
                    // Invalid float (e.g. NaN)
                    // Reset
                    window.clear();
                    sum_x = Decimal::ZERO;
                    sum_x2 = Decimal::ZERO;
                    middle_band.push(None);
                    upper_band.push(None);
                    lower_band.push(None);
                }
            }
            None => {
                // Missing data
                // Reset
                window.clear();
                sum_x = Decimal::ZERO;
                sum_x2 = Decimal::ZERO;
                middle_band.push(None);
                upper_band.push(None);
                lower_band.push(None);
            }
        }
    }

    let s_lower = Series::new("bollinger_lower", lower_band);
    let s_middle = Series::new("bollinger_middle", middle_band);
    let s_upper = Series::new("bollinger_upper", upper_band);

    Ok((s_lower, s_middle, s_upper))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        // Simple dataset: [10, 10, 10, 10, 10]
        // SMA(3) = 10
        // StdDev(3) = 0
        // Upper = 10, Lower = 10
        let df_stable = df!(
            "close" => &[10.0, 10.0, 10.0, 10.0, 10.0]
        )?;
        let (lower, middle, upper) = calculate(&df_stable, 3, 2.0)?;

        // Check middle band (SMA)
        let mid_vals = middle.f64()?;
        assert!(mid_vals.get(0).is_none());
        assert!(mid_vals.get(1).is_none());
        assert_eq!(mid_vals.get(2), Some(10.0));
        assert_eq!(mid_vals.get(3), Some(10.0));
        assert_eq!(mid_vals.get(4), Some(10.0));

        // Check bands
        let up_vals = upper.f64()?;
        let low_vals = lower.f64()?;
        assert_eq!(up_vals.get(2), Some(10.0));
        assert_eq!(low_vals.get(2), Some(10.0));

        // Dataset with variance: [10, 12, 14, 16, 18]
        // Period 3.
        // Index 2: [10, 12, 14]. Mean = 12.
        // Variance = ((10-12)^2 + (12-12)^2 + (14-12)^2) / 3 = (4 + 0 + 4)/3 = 8/3 = 2.666...
        // StdDev = sqrt(2.666) ≈ 1.63299
        // Upper = 12 + (2 * 1.63299) = 12 + 3.26598 = 15.26598
        // Lower = 12 - 3.26598 = 8.73402
        let df_var = df!(
            "close" => &[10.0, 12.0, 14.0, 16.0, 18.0]
        )?;
        let (lower_v, middle_v, upper_v) = calculate(&df_var, 3, 2.0)?;

        let m = middle_v.f64()?;
        let u = upper_v.f64()?;
        let l = lower_v.f64()?;

        assert_eq!(m.get(2), Some(12.0));

        // Use epsilon for float comparison
        let eps = 0.0001;
        let u_val = u.get(2).unwrap();
        let l_val = l.get(2).unwrap();

        assert!((u_val - 15.26598).abs() < eps, "Upper band mismatch: {} vs 15.26598", u_val);
        assert!((l_val - 8.73402).abs() < eps, "Lower band mismatch: {} vs 8.73402", l_val);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5, 2.0);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!("close" => &[10.0, 11.0])?;
        let (l, m, u) = calculate(&df_short, 5, 2.0)?;
        // Should return series of Nones
        assert!(m.f64()?.get(0).is_none());
        assert!(m.f64()?.get(1).is_none());

        assert!(l.f64()?.get(0).is_none());
        assert!(u.f64()?.get(0).is_none());

        assert_eq!(m.len(), 2);

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        // Just checking it runs without panic on a slightly larger set
        let values: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64)).collect();
        let df = df!("close" => values)?;

        let (l, m, u) = calculate(&df, 14, 2.0)?;
        assert_eq!(l.len(), 100);
        assert_eq!(m.len(), 100);
        assert_eq!(u.len(), 100);

        // Check first few are None
        for i in 0..13 {
            assert!(m.f64()?.get(i).is_none());
        }
        // Check 14th is valid
        assert!(m.f64()?.get(13).is_some());

        Ok(())
    }
}
