//! TSI - True Strength Index
//!
//! Calculates the True Strength Index (TSI), a momentum oscillator that measures the trend
//! and its strength. It uses double-smoothed exponential moving averages of price changes
//! to eliminate market noise and highlight the underlying trend.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use super::ema;

/// Calculate True Strength Index (TSI)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `long_period` - The first smoothing period (typically 25)
/// * `short_period` - The second smoothing period (typically 13)
///
/// # Returns
/// Series with TSI values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let tsi = strategies::indicators::tsi::calculate(&df, 25, 13)?;
/// ```
pub fn calculate(data: &DataFrame, long_period: usize, short_period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if long_period == 0 || short_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let mut pc_values: Vec<Option<f64>> = vec![None; close.len()];
    let mut abs_pc_values: Vec<Option<f64>> = vec![None; close.len()];

    // Calculate Price Change (PC) and Absolute Price Change (AbsPC)
    for i in 1..close.len() {
        let curr_opt = close.get(i);
        let prev_opt = close.get(i - 1);

        if let (Some(curr), Some(prev)) = (curr_opt, prev_opt) {
            let curr_dec_opt = Decimal::from_f64_retain(curr);
            let prev_dec_opt = Decimal::from_f64_retain(prev);

            if let (Some(curr_dec), Some(prev_dec)) = (curr_dec_opt, prev_dec_opt) {
                let pc = curr_dec - prev_dec;
                pc_values[i] = pc.to_f64();
                abs_pc_values[i] = pc.abs().to_f64();
            }
        }
    }

    let pc_series = Series::new("close", pc_values);
    let abs_pc_series = Series::new("close", abs_pc_values);

    let pc_df = DataFrame::new(vec![pc_series]).context("Failed to create temporary DataFrame for PC")?;
    let abs_pc_df = DataFrame::new(vec![abs_pc_series]).context("Failed to create temporary DataFrame for AbsPC")?;

    // First smoothing (EMA of long_period)
    let mut pc_ema1_series = ema::calculate(&pc_df, long_period)?;
    let mut abs_pc_ema1_series = ema::calculate(&abs_pc_df, long_period)?;

    let pc_ema1_df = DataFrame::new(vec![pc_ema1_series.rename("close").clone()])
        .context("Failed to create temporary DataFrame for PC EMA1")?;
    let abs_pc_ema1_df = DataFrame::new(vec![abs_pc_ema1_series.rename("close").clone()])
        .context("Failed to create temporary DataFrame for AbsPC EMA1")?;

    // Second smoothing (EMA of short_period)
    let pc_ema2_series = ema::calculate(&pc_ema1_df, short_period)?;
    let abs_pc_ema2_series = ema::calculate(&abs_pc_ema1_df, short_period)?;

    let pc_ema2_f64 = pc_ema2_series.f64().context("PC EMA2 must be numeric")?;
    let abs_pc_ema2_f64 = abs_pc_ema2_series.f64().context("AbsPC EMA2 must be numeric")?;

    let mut tsi_values: Vec<Option<f64>> = vec![None; close.len()];
    let hundred = Decimal::new(100, 0);

    for (i, tsi_val) in tsi_values.iter_mut().enumerate().take(close.len()) {
        let pc2_opt = pc_ema2_f64.get(i);
        let abs_pc2_opt = abs_pc_ema2_f64.get(i);

        if let (Some(pc2), Some(abs_pc2)) = (pc2_opt, abs_pc2_opt) {
            let pc2_dec_opt = Decimal::from_f64_retain(pc2);
            let abs_pc2_dec_opt = Decimal::from_f64_retain(abs_pc2);

            if let (Some(pc2_dec), Some(abs_pc2_dec)) = (pc2_dec_opt, abs_pc2_dec_opt) {
                if !abs_pc2_dec.is_zero() {
                    let tsi = (pc2_dec / abs_pc2_dec) * hundred;
                    *tsi_val = tsi.to_f64();
                } else {
                    *tsi_val = Some(0.0);
                }
            }
        }
    }

    Ok(Series::new("tsi", tsi_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!("close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0])?;
        let result = calculate(&df, 2, 2)?;
        let s = result.f64()?;
        assert_eq!(s.len(), 10);

        if let Some(val) = s.get(9) {
            assert!((val - 100.0).abs() < 1e-4, "TSI for pure uptrend should be 100, got {}", val);
        }

        let df_down = df!("close" => &[20.0, 19.0, 18.0, 17.0, 16.0, 15.0, 14.0, 13.0, 12.0, 11.0])?;
        let result_down = calculate(&df_down, 2, 2)?;
        let s_down = result_down.f64()?;

        if let Some(val) = s_down.get(9) {
            assert!((val + 100.0).abs() < 1e-4, "TSI for pure downtrend should be -100, got {}", val);
        }

        let df_flat = df!("close" => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0])?;
        let result_flat = calculate(&df_flat, 2, 2)?;
        let s_flat = result_flat.f64()?;

        if let Some(val) = s_flat.get(6) {
            assert!((val - 0.0).abs() < 1e-6, "TSI for flat price should be 0, got {}", val);
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 25, 13);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        let df_normal = df!("close" => &[10.0, 11.0])?;
        let res_zero = calculate(&df_normal, 0, 13);
        assert!(res_zero.is_err());

        let res_zero2 = calculate(&df_normal, 25, 0);
        assert!(res_zero2.is_err());

        let df_single = df!("close" => &[10.0])?;
        let res_single = calculate(&df_single, 25, 13)?;
        assert_eq!(res_single.len(), 1);
        assert!(res_single.f64()?.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100).map(|i| 10.0 + (i as f64 * 0.1).sin() * 5.0).collect();
        let df = df!("close" => values)?;

        let result = calculate(&df, 25, 13);
        assert!(result.is_ok());
        let s = result?;
        assert_eq!(s.len(), 100);

        let series = s.f64()?;
        assert!(series.get(0).is_none());

        let mut valid_count = 0;
        for i in 0..100 {
            if let Some(v) = series.get(i) {
                assert!((-100.0..=100.0).contains(&v), "TSI out of bounds: {}", v);
                valid_count += 1;
            }
        }

        assert!(valid_count > 0, "Expected some valid TSI values in 100 points");

        Ok(())
    }
}
