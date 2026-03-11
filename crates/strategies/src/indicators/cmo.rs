//! Chande Momentum Oscillator (CMO)
//!
//! Calculates the Chande Momentum Oscillator, which measures the momentum of a given asset.
//! It is calculated by taking the difference between the sum of recent gains and the sum of recent losses,
//! and dividing it by the sum of all price movements over the same period. The result is then multiplied by 100.
//! The oscillator oscillates between -100 and +100.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Chande Momentum Oscillator (CMO)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period (e.g., 9 or 14)
///
/// # Returns
/// Series with CMO values between -100 and 100.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use rust_decimal::Decimal;
/// use strategies::indicators::cmo;
///
/// let df = df!(
///     "close" => &[10.0, 11.0, 12.0, 11.0, 10.0]
/// ).unwrap_or_default();
/// let result = cmo::calculate(&df, 3).unwrap_or_default();
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let close_s = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .cast(&DataType::Float64)?;

    let close = close_s.f64().context("Close column must be numeric")?;

    let decimal_close: Vec<Option<Decimal>> = close
        .into_iter()
        .map(|opt_val| {
            if let Some(val) = opt_val {
                Decimal::from_f64_retain(val)
            } else {
                None
            }
        })
        .collect();

    let mut cmo_values: Vec<Option<f64>> = vec![None; decimal_close.len()];

    let hundred = Decimal::from(100);

    for (i, cmo_val) in cmo_values.iter_mut().enumerate().take(decimal_close.len()).skip(period) {
        let mut sum_gains = Decimal::ZERO;
        let mut sum_losses = Decimal::ZERO;
        let mut valid = true;

        for j in 0..period {
            let idx_curr = i - j;
            let idx_prev = idx_curr - 1;

            if let (Some(curr), Some(prev)) = (decimal_close[idx_curr], decimal_close[idx_prev]) {
                let diff = curr - prev;
                if diff > Decimal::ZERO {
                    sum_gains += diff;
                } else if diff < Decimal::ZERO {
                    // diff is negative, losses must be absolute positive
                    sum_losses -= diff;
                }
            } else {
                valid = false;
                break;
            }
        }

        if valid {
            let total_movement = sum_gains + sum_losses;
            if total_movement > Decimal::ZERO {
                let cmo = ((sum_gains - sum_losses) / total_movement) * hundred;
                *cmo_val = cmo.to_f64();
            } else {
                *cmo_val = Some(0.0);
            }
        }
    }

    Ok(Series::new("cmo", cmo_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_cmo_known_values() -> Result<()> {
        let df = df!(
            "close" => &[
                10.0, 11.0, 12.0, // 2 gains, 0 losses
                11.0, 10.0,       // 1 loss, 1 gain, 1 loss? -> previous was 12->11, 11->10
                10.0              // 0
            ]
        )?;

        // Period = 3, requires 3 differences (4 data points)
        // Data points:
        // idx 0: 10.0
        // idx 1: 11.0 (diff 1.0)
        // idx 2: 12.0 (diff 1.0)
        // idx 3: 11.0 (diff -1.0)  <- For i=3, diffs are (idx 3-idx 2) -1.0, (idx 2-idx 1) 1.0, (idx 1-idx 0) 1.0. Gains: 2, Losses: 1. CMO: (2-1)/(2+1) * 100 = 33.33
        // idx 4: 10.0 (diff -1.0)  <- For i=4, diffs are -1.0, -1.0, 1.0. Gains: 1, Losses: 2. CMO: (1-2)/(1+2) * 100 = -33.33
        // idx 5: 10.0 (diff 0.0)   <- For i=5, diffs are 0.0, -1.0, -1.0. Gains: 0, Losses: 2. CMO: (0-2)/(0+2) * 100 = -100

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());

        if let Some(val) = out.get(3) {
            assert!((val - 33.333333333333336).abs() < 1e-10);
        } else {
            anyhow::bail!("Missing value at index 3");
        }

        if let Some(val) = out.get(4) {
            assert!((val - -33.333333333333336).abs() < 1e-10);
        } else {
            anyhow::bail!("Missing value at index 4");
        }

        if let Some(val) = out.get(5) {
            assert!((val - -100.0).abs() < 1e-10);
        } else {
            anyhow::bail!("Missing value at index 5");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut closes = Vec::new();
        for i in 0..100 {
            closes.push(100.0 + (i as f64) * 0.5);
        }

        let df = df!("close" => &closes)?;
        let result = calculate(&df, 14)?;

        assert_eq!(result.len(), 100);
        let out = result.f64()?;
        assert!(out.get(13).is_none());

        if let Some(val) = out.get(14) {
            // Because it's monotonically increasing, gains=sum of diffs, losses=0
            // CMO should be 100.0
            assert!((val - 100.0).abs() < 1e-10);
        } else {
            anyhow::bail!("Missing value at index 14");
        }

        if let Some(val) = out.get(99) {
            assert!((val - 100.0).abs() < 1e-10);
        } else {
            anyhow::bail!("Missing value at index 99");
        }

        Ok(())
    }
}
