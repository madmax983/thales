//! Bollinger Bands - A volatility indicator consisting of a simple moving average (SMA) and two standard deviation bands.

use anyhow::{Context, Result};
use polars::prelude::*;

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

    // Contiguous run of valid values; reset on missing/NaN data.
    let mut run: Vec<f64> = Vec::with_capacity(close.len());
    let period_f = period as f64;

    for i in 0..close.len() {
        match close.get(i) {
            Some(val) if val.is_finite() => {
                run.push(val);
                if run.len() >= period {
                    // Two-pass over the window: avoids the cancellation a
                    // rolling sum of squares suffers in f64.
                    let window = &run[run.len() - period..];
                    let mean = window.iter().sum::<f64>() / period_f;
                    let variance =
                        window.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / period_f;
                    let std_dev = variance.sqrt();

                    middle_band.push(Some(mean));
                    upper_band.push(Some(mean + std_dev_multiplier * std_dev));
                    lower_band.push(Some(mean - std_dev_multiplier * std_dev));
                } else {
                    middle_band.push(None);
                    upper_band.push(None);
                    lower_band.push(None);
                }
            }
            _ => {
                run.clear();
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

        assert!(
            (u_val - 15.26598).abs() < eps,
            "Upper band mismatch: {} vs 15.26598",
            u_val
        );
        assert!(
            (l_val - 8.73402).abs() < eps,
            "Lower band mismatch: {} vs 8.73402",
            l_val
        );

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
