//! KDJ Indicator
//!
//! The KDJ Indicator is an extension of the Stochastic Oscillator.
//! It consists of three lines:
//! %K = Fast Stochastic
//! %D = Slow Stochastic (SMA of %K)
//! %J = 3 * %K - 2 * %D (Divergence of %K and %D)
//!
//! # Returns
//! Tuple of (Series %K, Series %D, Series %J).
//!
//! # Examples
//!
//! ```rust
//! use polars::prelude::*;
//! use strategies::indicators::kdj;
//!
//! let df = df!(
//!     "high" =>  &[10.0, 10.0, 10.0, 12.0],
//!     "low" =>   &[ 0.0,  0.0,  0.0,  2.0],
//!     "close" => &[ 5.0, 10.0,  0.0,  7.0]
//! ).unwrap();
//!
//! let (k, d, j) = kdj::calculate(&df, 2, 1, 2).unwrap();
//! assert_eq!(k.name(), "kdj_k");
//! assert_eq!(d.name(), "kdj_d");
//! assert_eq!(j.name(), "kdj_j");
//! ```

use anyhow::Result;
use polars::prelude::*;

use crate::indicators::stochastic;

/// Calculate KDJ Indicator
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns
/// * `k_period` - Lookback period for %K (e.g., 9)
/// * `k_smoothing` - Smoothing period for %K (e.g., 3)
/// * `d_period` - Smoothing period for %D (e.g., 3)
///
/// # Returns
/// Tuple of (Series %K, Series %D, Series %J). First few values will be null.
/// Series names: "kdj_k", "kdj_d", "kdj_j"
pub fn calculate(
    data: &DataFrame,
    k_period: usize,
    k_smoothing: usize,
    d_period: usize,
) -> Result<(Series, Series, Series)> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    // Reuse stochastic calculation for %K and %D
    let (k_series, d_series) = stochastic::calculate(data, k_period, k_smoothing, d_period)?;

    let k_arr = k_series.f64()?;
    let d_arr = d_series.f64()?;

    let mut j_f64: Vec<Option<f64>> = vec![None; data.height()];

    for (i, j_val) in j_f64.iter_mut().enumerate().take(data.height()) {
        if let (Some(k), Some(d)) = (k_arr.get(i), d_arr.get(i)) {
            // %J = 3 * %K - 2 * %D
            let j = 3.0 * k - 2.0 * d;
            *j_val = Some(j);
        }
    }

    let mut k_series_renamed = k_series.clone();
    k_series_renamed.rename("kdj_k");
    let mut d_series_renamed = d_series.clone();
    d_series_renamed.rename("kdj_d");
    let j_series = Series::new("kdj_j", j_f64);

    Ok((k_series_renamed, d_series_renamed, j_series))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_kdj_calculation() -> Result<()> {
        let df = df!(
            "high" =>  &[10.0, 10.0, 10.0, 12.0],
            "low" =>   &[ 0.0,  0.0,  0.0,  2.0],
            "close" => &[ 5.0, 10.0,  0.0,  7.0]
        )?;

        let (k, d, j) = calculate(&df, 2, 1, 2)?;
        let k_vals = k.f64()?;
        let d_vals = d.f64()?;
        let j_vals = j.f64()?;

        // Index 2
        let val_k2 = k_vals.get(2).unwrap();
        assert!((val_k2 - 0.0).abs() < 0.001);

        let val_d2 = d_vals.get(2).unwrap();
        assert!((val_d2 - 50.0).abs() < 0.001);

        let val_j2 = j_vals.get(2).unwrap();
        // J = 3 * 0 - 2 * 50 = -100
        assert!((val_j2 - (-100.0)).abs() < 0.001);

        // Index 3
        let val_k3 = k_vals.get(3).unwrap();
        assert!((val_k3 - 58.333).abs() < 0.001);

        let val_d3 = d_vals.get(3).unwrap();
        assert!((val_d3 - 29.166).abs() < 0.001);

        let val_j3 = j_vals.get(3).unwrap();
        // J = 3 * 58.333 - 2 * 29.166 = 175.0 - 58.332 = 116.668
        assert!((val_j3 - 116.666).abs() < 0.005);

        Ok(())
    }

    #[test]
    fn test_empty_data() {
        let df_empty = DataFrame::default();
        let res = calculate(&df_empty, 9, 3, 3);
        assert!(res.is_err());
    }
}
