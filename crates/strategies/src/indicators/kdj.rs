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

use anyhow::Result;

use polars::prelude::*;
use rust_decimal::prelude::*;

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
///
/// # Example
/// ```rust
/// use strategies::indicators::kdj;
/// use polars::prelude::*;
/// // let df = ...;
/// // let (k, d, j) = kdj::calculate(&df, 9, 3, 3)?;
/// ```
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
    // stochastic::calculate natively calculates Decimal and converts to f64 Series at the very end
    let (k_series, d_series) = stochastic::calculate(data, k_period, k_smoothing, d_period)?;

    let k_arr = k_series.f64()?;
    let d_arr = d_series.f64()?;

    let mut j_f64: Vec<Option<f64>> = vec![None; data.height()];
    let three = Decimal::new(3, 0);
    let two = Decimal::new(2, 0);

    for (i, j_val) in j_f64.iter_mut().enumerate().take(data.height()) {
        if let (Some(k), Some(d)) = (k_arr.get(i), d_arr.get(i)) {
            // Use Decimal for financial calculations
            let k_dec = Decimal::from_f64_retain(k).unwrap_or(Decimal::ZERO);
            let d_dec = Decimal::from_f64_retain(d).unwrap_or(Decimal::ZERO);

            // %J = 3 * %K - 2 * %D
            let j_dec = (three * k_dec) - (two * d_dec);
            *j_val = j_dec.to_f64();
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
    fn test_known_values() -> Result<()> {
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
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 9, 3, 3);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!(
            "high" =>  &[10.0, 11.0, 12.0],
            "low" =>   &[ 9.0, 10.0, 11.0],
            "close" => &[ 9.5, 10.5, 11.5]
        )?;
        let (k_short, _, _) = calculate(&df_short, 5, 3, 3)?;
        let out = k_short.f64()?;
        assert_eq!(out.len(), 3);
        assert!(out.get(0).is_none());
        assert!(out.get(2).is_none());

        // Single data point
        let df_single = df!(
            "high" =>  &[10.0],
            "low" =>   &[ 9.0],
            "close" => &[ 9.5]
        )?;
        let (k_single, _, _) = calculate(&df_single, 9, 3, 3)?;
        let out_single = k_single.f64()?;
        assert_eq!(out_single.len(), 1);
        assert!(out_single.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let highs: Vec<f64> = values.iter().map(|v| v + 1.0).collect();
        let lows: Vec<f64> = values.iter().map(|v| v - 1.0).collect();
        let closes = values;

        let df = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let result = calculate(&df, 14, 3, 3);
        assert!(result.is_ok());
        let (k, d, j) = result?;
        assert_eq!(k.len(), 100);
        assert_eq!(d.len(), 100);
        assert_eq!(j.len(), 100);

        let k_series = k.f64()?;
        for i in 20..100 {
            if let Some(v) = k_series.get(i) {
                assert!(
                    (0.0..=100.0).contains(&v),
                    "K {} out of bounds at {}",
                    v,
                    i
                );
            }
        }

        Ok(())
    }
}
