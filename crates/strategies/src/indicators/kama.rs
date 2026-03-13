//! Kaufman's Adaptive Moving Average (KAMA)
//!
//! Calculates the KAMA, an intelligent moving average that adapts to market volatility.
//! It moves closely with prices when noise is low, and smooths out the trend when noise is high.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Kaufman's Adaptive Moving Average (KAMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Efficiency Ratio (ER) lookback period (typically 10)
/// * `fast_period` - Fast EMA smoothing period (typically 2)
/// * `slow_period` - Slow EMA smoothing period (typically 30)
///
/// # Returns
/// Series with KAMA values. The first `period` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let kama = strategies::indicators::kama::calculate(&df, 10, 2, 30)?;
/// ```
pub fn calculate(
    data: &DataFrame,
    period: usize,
    fast_period: usize,
    slow_period: usize,
) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 || fast_period == 0 || slow_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }
    if fast_period >= slow_period {
        anyhow::bail!("fast_period must be less than slow_period");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let len = close.len();
    let mut kama_values: Vec<Option<f64>> = vec![None; len];

    if len <= period {
        return Ok(Series::new("kama", kama_values));
    }

    let fast_sc = Decimal::new(2, 0)
        / Decimal::from_usize(fast_period + 1).context("fast_period + 1 conversion failed")?;
    let slow_sc = Decimal::new(2, 0)
        / Decimal::from_usize(slow_period + 1).context("slow_period + 1 conversion failed")?;

    let mut prev_kama: Option<Decimal> = None;

    // Calculate absolute price changes for volatility sum.
    let mut abs_changes = Vec::with_capacity(len);
    for i in 0..len {
        let curr = close.get(i);
        let prev = if i > 0 { close.get(i - 1) } else { None };

        if let (Some(c), Some(p)) = (curr, prev) {
            let c_dec = Decimal::from_f64(c).unwrap_or(Decimal::ZERO);
            let p_dec = Decimal::from_f64(p).unwrap_or(Decimal::ZERO);
            abs_changes.push(Some((c_dec - p_dec).abs()));
        } else {
            abs_changes.push(None);
        }
    }

    // Maintain a rolling sum of the past `period` absolute changes
    let mut current_volatility = Decimal::ZERO;
    let mut valid_changes_count = 0;

    // Initialize the rolling sum for the first `period` window.
    for j in 1..=period {
        if period < len {
            if let Some(Some(change)) = abs_changes.get(period - j + 1) {
                current_volatility += change;
                valid_changes_count += 1;
            }
        }
    }

    for (i, kama_val) in kama_values.iter_mut().enumerate().take(len).skip(period) {
        let current_close = if let Some(c) = close.get(i) {
            Decimal::from_f64(c).unwrap_or(Decimal::ZERO)
        } else {
            continue;
        };

        if prev_kama.is_none() {
            if let Some(prev_c) = close.get(i - 1) {
                prev_kama = Some(Decimal::from_f64(prev_c).unwrap_or(Decimal::ZERO));
            } else {
                continue;
            }
        }

        let price_i_minus_period = if let Some(p) = close.get(i - period) {
            Decimal::from_f64(p).unwrap_or(Decimal::ZERO)
        } else {
            continue;
        };

        // Update volatility sum (O(1))
        // Add new change
        if let Some(Some(new_change)) = abs_changes.get(i) {
            current_volatility += new_change;
            valid_changes_count += 1;
        }
        // Remove old change
        if let Some(Some(old_change)) = abs_changes.get(i - period) {
            current_volatility -= old_change;
            valid_changes_count -= 1;
        }

        if valid_changes_count < period {
            continue;
        }

        let change = (current_close - price_i_minus_period).abs();

        let er = if current_volatility.is_zero() {
            Decimal::ZERO
        } else {
            change / current_volatility
        };

        let sc = (er * (fast_sc - slow_sc) + slow_sc).powi(2);

        let pk = prev_kama.context("prev_kama should be some")?;
        let current_kama = pk + sc * (current_close - pk);

        *kama_val = Some(
            current_kama
                .to_f64()
                .context("current_kama to_f64 failed")?,
        );
        prev_kama = Some(current_kama);
    }

    Ok(Series::new("kama", kama_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 11.0, 13.0, 15.0]
        )?;

        // Period 3. Fast 2, Slow 30
        let result = calculate(&df, 3, 2, 30)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 6);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());

        assert!(out.get(3).is_some());
        assert!(out.get(4).is_some());
        assert!(out.get(5).is_some());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 10, 2, 30);
        assert!(res_empty.is_err());

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5, 2, 30)?;
        let out = res_short.f64()?;
        assert_eq!(out.len(), 2);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let df_invalid_params = df!("close" => &[10.0, 11.0, 12.0])?;
        assert!(calculate(&df_invalid_params, 0, 2, 30).is_err());
        assert!(calculate(&df_invalid_params, 10, 0, 30).is_err());
        assert!(calculate(&df_invalid_params, 10, 2, 0).is_err());
        assert!(calculate(&df_invalid_params, 10, 30, 2).is_err()); // fast >= slow

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..50).map(|i| 100.0 + (i as f64)).collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 10, 2, 30)?;
        let s = result.f64()?;

        assert_eq!(s.len(), 50);
        assert!(s.get(9).is_none());
        assert!(s.get(10).is_some());

        let last_val = s.get(49).unwrap();
        assert!(last_val > 100.0);

        Ok(())
    }
}
