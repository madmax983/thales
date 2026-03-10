//! True Strength Index (TSI)
//!
//! The True Strength Index (TSI) is a momentum oscillator based on a double EMA of price changes.
//! It is used to identify trend direction and overbought/oversold conditions.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use super::ema;

/// Calculate True Strength Index (TSI)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `long_period` - The first EMA period (typically 25)
/// * `short_period` - The second EMA period (typically 13)
///
/// # Returns
/// Series with TSI values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let tsi_series = strategies::indicators::tsi::calculate(&df, 25, 13)?;
/// ```
pub fn calculate(data: &DataFrame, long_period: usize, short_period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if long_period == 0 || short_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

    let close_series = data
        .column("close")
        .context("DataFrame must contain 'close' column")?;

    // Calculate Momentum (Close - Prev Close)
    let mut momentum_series = (close_series - &close_series.shift(1))?;

    // Absolute Momentum
    let mut abs_momentum_series = momentum_series.f64()?.apply(|opt_val| opt_val.map(|v| v.abs())).into_series();

    let momentum_df = DataFrame::new(vec![momentum_series.rename("close".into()).clone()])?;
    let abs_momentum_df = DataFrame::new(vec![abs_momentum_series.rename("close".into()).clone()])?;

    // EMA 1: long_period
    let mut ema_mom_1 = ema::calculate(&momentum_df, long_period)?;
    let mut ema_abs_1 = ema::calculate(&abs_momentum_df, long_period)?;

    let ema_mom_1_df = DataFrame::new(vec![ema_mom_1.rename("close".into()).clone()])?;
    let ema_abs_1_df = DataFrame::new(vec![ema_abs_1.rename("close".into()).clone()])?;

    // EMA 2: short_period
    let ema_mom_2 = ema::calculate(&ema_mom_1_df, short_period)?;
    let ema_abs_2 = ema::calculate(&ema_abs_1_df, short_period)?;

    let ema_mom_2_f64 = ema_mom_2.f64()?;
    let ema_abs_2_f64 = ema_abs_2.f64()?;

    let hundred = Decimal::new(100, 0);
    let mut tsi_vals = Vec::with_capacity(close_series.len());

    for i in 0..close_series.len() {
        if let (Some(mom), Some(abs_mom)) = (ema_mom_2_f64.get(i), ema_abs_2_f64.get(i)) {
            if let (Some(mom_dec), Some(abs_mom_dec)) = (Decimal::from_f64_retain(mom), Decimal::from_f64_retain(abs_mom)) {
                if !abs_mom_dec.is_zero() {
                    let tsi = (mom_dec / abs_mom_dec) * hundred;
                    tsi_vals.push(tsi.to_f64());
                } else {
                    tsi_vals.push(Some(0.0));
                }
            } else {
                tsi_vals.push(None);
            }
        } else {
            tsi_vals.push(None);
        }
    }

    Ok(Series::new("tsi", tsi_vals))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 25, 13);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        let df_single = df!("close" => &[10.0])?;
        assert!(calculate(&df_single, 0, 13).is_err());
        assert!(calculate(&df_single, 25, 0).is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64 * 0.5).sin() * 10.0).collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 25, 13)?;
        let s = result.f64()?;

        assert_eq!(s.len(), 100);
        assert!(s.get(0).is_none());
        assert!(s.get(99).is_some());

        Ok(())
    }

    #[test]
    fn test_known_values() -> Result<()> {
        // Simple manual calculation for known values verification
        // Data: 10, 12, 11, 14, 13
        // Momentum: None, 2, -1, 3, -1
        // Abs Momentum: None, 2, 1, 3, 1

        // Let's use EMA period 2 for both long and short for a quicker test
        let df = df!("close" => &[10.0, 12.0, 11.0, 14.0, 13.0])?;
        let result = calculate(&df, 2, 2)?;
        let s = result.f64()?;

        // Length should match
        assert_eq!(s.len(), 5);

        // First index is missing because of Momentum (shift 1) and then EMA (needs 2 values for SMA seed)
        // With EMA(period=2), EMA begins output at index 1 of its input.
        // Momentum has valid values starting at index 1.
        // EMA1(mom) valid starting at index 1 of momentum (which is index 2 of original array).
        // EMA2(EMA1) valid starting at index 1 of EMA1 (which is index 3 of original array).
        // Let's verify value at index 3.

        // Manual trace:
        // Mom: [None, 2.0, -1.0, 3.0, -1.0]
        // AbsMom: [None, 2.0, 1.0, 3.0, 1.0]

        // EMA1(Mom, 2):
        //   idx 0 (original idx 1): 2.0
        //   idx 1 (original idx 2): SMA(2, -1) = 0.5. Wait, SMA of first 2 values [2.0, -1.0] = 0.5
        //   idx 2 (original idx 3): EMA = (3.0 * (2/3)) + (0.5 * (1/3)) = 2.0 + 0.16666 = 2.16666

        // EMA1(AbsMom, 2):
        //   idx 0 (original idx 1): 2.0
        //   idx 1 (original idx 2): SMA(2, 1) = 1.5
        //   idx 2 (original idx 3): EMA = (3.0 * (2/3)) + (1.5 * (1/3)) = 2.0 + 0.5 = 2.5

        // EMA2(EMA1_Mom, 2):
        //   EMA1_Mom: [None, None, 0.5, 2.16666, ...]
        //   SMA(0.5, 2.16666) = 1.333333 (at index 3)

        // EMA2(EMA1_AbsMom, 2):
        //   EMA1_AbsMom: [None, None, 1.5, 2.5, ...]
        //   SMA(1.5, 2.5) = 2.0 (at index 3)

        // TSI at index 3 = (1.333333 / 2.0) * 100 = 66.66665

        // The values will be approx 66.66665.
        // We will assert within a tiny epsilon.

        if let Some(val) = s.get(3) {
            let expected = 66.66666666666666;
            assert!((val - expected).abs() < 1e-4, "Expected approx 66.6667, got {}", val);
        } else {
            anyhow::bail!("Expected value at index 3");
        }

        Ok(())
    }
}
