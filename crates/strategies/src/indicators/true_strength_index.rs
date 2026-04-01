//! TSI - True Strength Index
//!
//! Calculates the True Strength Index (TSI), a momentum oscillator based on a double
//! smoothed exponential moving average of price momentum.
//!
//! Formula:
//! PC = Current Close - Prior Close
//! Double Smoothed PC = EMA(EMA(PC, Long Period), Short Period)
//! Double Smoothed Absolute PC = EMA(EMA(|PC|, Long Period), Short Period)
//! TSI = 100 * (Double Smoothed PC / Double Smoothed Absolute PC)

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate True Strength Index (TSI)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `long_period` - First EMA smoothing period (typically 25)
/// * `short_period` - Second EMA smoothing period (typically 13)
///
/// # Returns
/// Series with TSI values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let tsi = strategies::indicators::true_strength_index::calculate(&df, 25, 13)?;
/// ```
pub fn calculate(data: &DataFrame, long_period: usize, short_period: usize) -> Result<Series> {
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

    let len = close.len();
    let mut pc = vec![None; len];
    let mut abs_pc = vec![None; len];

    for i in 1..len {
        let curr_opt = close.get(i);
        let prev_opt = close.get(i - 1);

        if let (Some(curr), Some(prev)) = (curr_opt, prev_opt) {
            if let (Some(c), Some(p)) = (Decimal::from_f64_retain(curr), Decimal::from_f64_retain(prev)) {
                let change = c - p;
                pc[i] = Some(change);
                abs_pc[i] = Some(change.abs());
            }
        }
    }

    let ema_pc1 = calculate_ema(&pc, long_period)?;
    let ema_pc2 = calculate_ema(&ema_pc1, short_period)?;

    let ema_abs_pc1 = calculate_ema(&abs_pc, long_period)?;
    let ema_abs_pc2 = calculate_ema(&ema_abs_pc1, short_period)?;

    let mut tsi_values: Vec<Option<f64>> = vec![None; len];
    let hundred = Decimal::new(100, 0);

    for i in 0..len {
        if let (Some(num), Some(den)) = (ema_pc2[i], ema_abs_pc2[i]) {
            if den.is_zero() {
                tsi_values[i] = Some(0.0);
            } else {
                let tsi = hundred * (num / den);
                tsi_values[i] = tsi.to_f64();
            }
        }
    }

    Ok(Series::new("tsi", tsi_values))
}

fn calculate_ema(data: &[Option<Decimal>], period: usize) -> Result<Vec<Option<Decimal>>> {
    let len = data.len();
    let mut result = vec![None; len];

    if len == 0 || period == 0 {
        return Ok(result);
    }

    let period_dec = Decimal::from_usize(period).context("Invalid period conversion")?;
    let multiplier = Decimal::new(2, 0) / (period_dec + Decimal::ONE);
    let one_minus_multiplier = Decimal::ONE - multiplier;

    let mut ema_prev: Option<Decimal> = None;
    let mut sum = Decimal::ZERO;
    let mut count = 0;

    for i in 0..len {
        if let Some(val) = data[i] {
            if ema_prev.is_none() {
                sum += val;
                count += 1;

                if count == period {
                    let initial_sma = sum / period_dec;
                    ema_prev = Some(initial_sma);
                    result[i] = ema_prev;
                }
            } else if let Some(prev) = ema_prev {
                let current_ema = (val * multiplier) + (prev * one_minus_multiplier);
                ema_prev = Some(current_ema);
                result[i] = ema_prev;
            }
        } else {
            // Gap in data, reset
            ema_prev = None;
            sum = Decimal::ZERO;
            count = 0;
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 12.0, 11.0, 13.0, 15.0, 14.0, 16.0]
        )?;

        let tsi = calculate(&df, 2, 2)?;
        let tsi_arr = tsi.f64()?;

        assert_eq!(tsi_arr.len(), 7);
        // PC: None, 2.0, -1.0, 2.0, 2.0, -1.0, 2.0
        // Abs PC: None, 2.0, 1.0, 2.0, 2.0, 1.0, 2.0
        // EMA logic needs at least `period` elements before calculating first value.
        // PC[1..3] = [2.0, -1.0], SMA = 0.5 (at index 2)
        // PC[3] = 2.0. EMA = (2.0 * 2/3) + (0.5 * 1/3) = 1.33 + 0.16 = 1.5

        let v = tsi_arr.get(5);
        if let Some(val) = v {
            assert!((val - 32.231).abs() < 1e-3, "Expected ~32.231, got {}", val);
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res = calculate(&df_empty, 25, 13);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_short = calculate(&df_short, 25, 13)?;
        let tsi = res_short.f64()?;
        assert_eq!(tsi.len(), 3);
        assert!(tsi.get(2).is_none());

        let res_zero = calculate(&df_short, 0, 13);
        assert!(res_zero.is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let df = df!("close" => values)?;
        let res = calculate(&df, 25, 13)?;
        let tsi = res.f64()?;
        assert_eq!(tsi.len(), 100);

        for i in 50..100 {
            if let Some(v) = tsi.get(i) {
                assert!(((-100.0)..=100.0).contains(&v), "TSI out of bounds: {}", v);
            }
        }

        Ok(())
    }
}
