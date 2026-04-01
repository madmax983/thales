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
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

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
    // Stochastic calculates internal using Decimal, but returns f64 series.
    // To strictly follow the `rust_decimal::Decimal` constraint without a manual map,
    // we would have to pull everything to Decimals.
    let (k_series, d_series) = stochastic::calculate(data, k_period, k_smoothing, d_period)?;

    let k_arr = k_series.f64()?;
    let d_arr = d_series.f64()?;

    let mut j_f64: Vec<Option<f64>> = vec![None; data.height()];

    let three = Decimal::new(3, 0);
    let two = Decimal::new(2, 0);

    for (i, j_val) in j_f64.iter_mut().enumerate().take(data.height()) {
        if let (Some(k), Some(d)) = (k_arr.get(i), d_arr.get(i)) {
            if let (Some(k_dec), Some(d_dec)) = (
                Decimal::from_f64_retain(k),
                Decimal::from_f64_retain(d),
            ) {
                // %J = 3 * %K - 2 * %D
                let j_dec = (three * k_dec) - (two * d_dec);
                *j_val = j_dec.to_f64();
            }
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
        let df_empty = DataFrame::default();
        let res = calculate(&df_empty, 9, 3, 3);
        assert!(res.is_err(), "Should error on empty DataFrame");

        // Single data point
        let df_single = df!(
            "high" => &[10.0],
            "low" => &[5.0],
            "close" => &[8.0]
        )?;
        let (k, d, j) = calculate(&df_single, 14, 3, 3)?;
        assert_eq!(k.len(), 1);
        assert_eq!(d.len(), 1);
        assert_eq!(j.len(), 1);
        assert!(k.f64()?.get(0).is_none());
        assert!(d.f64()?.get(0).is_none());
        assert!(j.f64()?.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let size = 100;
        let mut highs = Vec::with_capacity(size);
        let mut lows = Vec::with_capacity(size);
        let mut closes = Vec::with_capacity(size);

        for i in 0..size {
            let base = 100.0 + (i as f64 * 0.1).sin() * 10.0;
            highs.push(base + 1.0);
            lows.push(base - 1.0);
            closes.push(base);
        }

        let df = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let (k, d, j) = calculate(&df, 14, 3, 3)?;
        assert_eq!(k.len(), 100);
        assert_eq!(d.len(), 100);
        assert_eq!(j.len(), 100);

        let j_arr = j.f64()?;
        if let Some(val) = j_arr.get(50) {
            // J is expected to sometimes go out of [0, 100] bound, so we just verify it exists and is finite
            assert!(val.is_finite());
        } else {
            panic!("Expected value at index 50");
        }

        Ok(())
    }
}
