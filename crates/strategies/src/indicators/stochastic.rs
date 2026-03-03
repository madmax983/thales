//! Stochastic Oscillator
//!
//! The Stochastic Oscillator is a momentum indicator comparing a particular closing
//! price of a security to a range of its prices over a certain period of time.
//! It consists of two lines:
//! - `%K`: The fast indicator line.
//! - `%D`: The slow indicator line, which is a moving average of `%K`.

use super::sma;
use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;

/// Calculate the Stochastic Oscillator (%K and %D)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", and "close" columns
/// * `k_period` - Lookback period for %K (standard is 14)
/// * `d_period` - Smoothing period for %D (standard is 3)
///
/// # Returns
/// A tuple containing two Series: (%K, %D).
/// The first `k_period - 1` values of %K will be null.
/// The %D will have additional initial nulls depending on `d_period`.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let (k, d) = strategies::indicators::stochastic::calculate(&df, 14, 3)?;
/// ```
pub fn calculate(data: &DataFrame, k_period: usize, d_period: usize) -> Result<(Series, Series)> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if k_period == 0 {
        anyhow::bail!("k_period must be greater than 0");
    }
    if d_period == 0 {
        anyhow::bail!("d_period must be greater than 0");
    }

    // Use LazyFrame for vectorized operations
    let lazy_df = data.clone().lazy();

    let result_df = lazy_df
        .with_columns([
            col("high")
                .rolling_max(RollingOptionsFixedWindow {
                    window_size: k_period,
                    min_periods: k_period,
                    weights: None,
                    center: false,
                    fn_params: None,
                })
                .alias("highest_high"),
            col("low")
                .rolling_min(RollingOptionsFixedWindow {
                    window_size: k_period,
                    min_periods: k_period,
                    weights: None,
                    center: false,
                    fn_params: None,
                })
                .alias("lowest_low"),
        ])
        .collect()
        .context("Failed to compute rolling min/max")?;

    let close_col = result_df.column("close")?.f64()?;
    let hh_col = result_df.column("highest_high")?.f64()?;
    let ll_col = result_df.column("lowest_low")?.f64()?;

    let len = close_col.len();
    let mut k_values: Vec<Option<f64>> = vec![None; len];

    let hundred = Decimal::new(100, 0);
    let fifty = Decimal::new(50, 0);

    for i in 0..len {
        if let (Some(c), Some(hh), Some(ll)) = (close_col.get(i), hh_col.get(i), ll_col.get(i)) {
            let c_dec = Decimal::from_f64_retain(c).unwrap_or(Decimal::ZERO);
            let hh_dec = Decimal::from_f64_retain(hh).unwrap_or(Decimal::ZERO);
            let ll_dec = Decimal::from_f64_retain(ll).unwrap_or(Decimal::ZERO);

            let range = hh_dec - ll_dec;

            let k = if range.is_zero() {
                fifty
            } else {
                ((c_dec - ll_dec) / range) * hundred
            };

            k_values[i] = Some(k.to_f64().unwrap_or(50.0));
        }
    }

    let k_series = Series::new("stoch_k", &k_values);

    let temp_df = DataFrame::new(vec![Series::new("close", &k_series)])?;
    let d_series = sma::calculate(&temp_df, d_period).context("Failed to calculate %D SMA")?;

    let mut d_renamed = d_series;
    d_renamed.rename("stoch_d");

    Ok((k_series, d_renamed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        // Simple manual calculation test
        // Let's use k_period = 3, d_period = 2
        //
        // Data points (high, low, close):
        // 0: H=10, L=8, C=9
        // 1: H=12, L=9, C=11
        // 2: H=15, L=10, C=14
        //    Lowest Low (3) = min(8,9,10) = 8
        //    Highest High (3) = max(10,12,15) = 15
        //    %K[2] = (14 - 8) / (15 - 8) * 100 = 6 / 7 * 100 = 85.714
        //
        // 3: H=14, L=12, C=13
        //    Lowest Low (3) = min(9,10,12) = 9
        //    Highest High (3) = max(12,15,14) = 15
        //    %K[3] = (13 - 9) / (15 - 9) * 100 = 4 / 6 * 100 = 66.666
        //    %D[3] = SMA(%K, 2) = (85.714 + 66.666) / 2 = 76.19
        //
        // 4: H=16, L=13, C=15
        //    Lowest Low (3) = min(10,12,13) = 10
        //    Highest High (3) = max(15,14,16) = 16
        //    %K[4] = (15 - 10) / (16 - 10) * 100 = 5 / 6 * 100 = 83.333
        //    %D[4] = SMA(%K, 2) = (66.666 + 83.333) / 2 = 75.00

        let df = df!(
            "high" => &[10.0, 12.0, 15.0, 14.0, 16.0],
            "low" => &[8.0, 9.0, 10.0, 12.0, 13.0],
            "close" => &[9.0, 11.0, 14.0, 13.0, 15.0]
        )?;

        let (k_series, d_series) = calculate(&df, 3, 2)?;

        let k_vals = k_series.f64()?;
        let d_vals = d_series.f64()?;

        assert_eq!(k_vals.len(), 5);
        assert!(k_vals.get(0).is_none());
        assert!(k_vals.get(1).is_none());

        // Check %K
        let k2 = k_vals.get(2).context("Missing %K at 2")?;
        assert!((k2 - 85.714285).abs() < 1e-4, "Expected ~85.71, got {}", k2);
        let k3 = k_vals.get(3).context("Missing %K at 3")?;
        assert!((k3 - 66.666666).abs() < 1e-4, "Expected ~66.67, got {}", k3);
        let k4 = k_vals.get(4).context("Missing %K at 4")?;
        assert!((k4 - 83.333333).abs() < 1e-4, "Expected ~83.33, got {}", k4);

        // Check %D
        assert!(d_vals.get(2).is_none());
        let d3 = d_vals.get(3).context("Missing %D at 3")?;
        assert!((d3 - 76.190476).abs() < 1e-4, "Expected ~76.19, got {}", d3);
        let d4 = d_vals.get(4).context("Missing %D at 4")?;
        assert!((d4 - 75.0).abs() < 1e-4, "Expected ~75.0, got {}", d4);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14, 3);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        // Single data point
        let df_single = df!(
            "high" => &[10.0],
            "low" => &[8.0],
            "close" => &[9.0]
        )?;
        let (k_single, d_single) = calculate(&df_single, 14, 3)?;
        assert_eq!(k_single.len(), 1);
        assert!(k_single.f64()?.get(0).is_none());
        assert_eq!(d_single.len(), 1);
        assert!(d_single.f64()?.get(0).is_none());

        // Zero period
        let df_normal = df!(
            "high" => &[10.0, 11.0],
            "low" => &[8.0, 9.0],
            "close" => &[9.0, 10.0]
        )?;
        let res_zero_k = calculate(&df_normal, 0, 3);
        assert!(res_zero_k.is_err());
        if let Err(e) = res_zero_k {
            assert_eq!(e.to_string(), "k_period must be greater than 0");
        }

        let res_zero_d = calculate(&df_normal, 3, 0);
        assert!(res_zero_d.is_err());
        if let Err(e) = res_zero_d {
            assert_eq!(e.to_string(), "d_period must be greater than 0");
        }

        // High == Low (division by zero prevention)
        let df_flat = df!(
            "high" => &[10.0, 10.0, 10.0],
            "low" => &[10.0, 10.0, 10.0],
            "close" => &[10.0, 10.0, 10.0]
        )?;
        let (k_flat, _d_flat) = calculate(&df_flat, 2, 2)?;
        let k_flat_vals = k_flat.f64()?;
        assert!(k_flat_vals.get(0).is_none());
        // Standard stochastic usually outputs 50 or 0 or 100 when range is 0.
        // We will output 50 to avoid NaN/Infinity and match RSI/neutral behavior.
        assert_eq!(k_flat_vals.get(1).context("Missing %K at 1")?, 50.0);

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
            highs.push(base + 2.0);
            lows.push(base - 2.0);
            closes.push(base); // exactly in the middle
        }

        let df = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let (k_series, d_series) = calculate(&df, 14, 3)?;

        assert_eq!(k_series.len(), 100);
        assert_eq!(d_series.len(), 100);

        let k_vals = k_series.f64()?;
        let d_vals = d_series.f64()?;

        assert!(k_vals.get(12).is_none()); // 0..12 = 13 items
        assert!(k_vals.get(13).is_some()); // 14th item

        assert!(d_vals.get(14).is_none());
        assert!(d_vals.get(15).is_some()); // 14th + 2 items

        // Check bounds
        for i in 13..size {
            let k = k_vals.get(i).context("Missing %K value")?;
            assert!(k >= 0.0 && k <= 100.0, "%K out of bounds: {}", k);
        }
        for i in 15..size {
            let d = d_vals.get(i).context("Missing %D value")?;
            assert!(d >= 0.0 && d <= 100.0, "%D out of bounds: {}", d);
        }

        Ok(())
    }
}
