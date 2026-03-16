//! KST - Know Sure Thing
//!
//! Calculates the Know Sure Thing (KST) oscillator, a momentum oscillator developed by Martin Pring.
//! It is based on the smoothed rate of change (ROC) for four different timeframes.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Know Sure Thing (KST)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `roc1` - Lookback period for ROC 1
/// * `roc2` - Lookback period for ROC 2
/// * `roc3` - Lookback period for ROC 3
/// * `roc4` - Lookback period for ROC 4
/// * `sma1` - Smoothing period for ROC 1
/// * `sma2` - Smoothing period for ROC 2
/// * `sma3` - Smoothing period for ROC 3
/// * `sma4` - Smoothing period for ROC 4
///
/// # Returns
/// Series with KST values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::kst;
///
/// let df = df!(
///     "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 24.0]
/// ).unwrap_or_default();
/// // Using short periods for example
/// let result = kst::calculate(&df, 3, 4, 5, 6, 3, 3, 3, 4).unwrap_or_default();
/// ```
pub fn calculate(
    data: &DataFrame,
    roc1: usize,
    roc2: usize,
    roc3: usize,
    roc4: usize,
    sma1: usize,
    sma2: usize,
    sma3: usize,
    sma4: usize,
) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if roc1 == 0 || roc2 == 0 || roc3 == 0 || roc4 == 0 {
        anyhow::bail!("ROC periods must be greater than 0");
    }
    if sma1 == 0 || sma2 == 0 || sma3 == 0 || sma4 == 0 {
        anyhow::bail!("SMA periods must be greater than 0");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let close_vec: Vec<Option<Decimal>> = close
        .into_iter()
        .map(|opt_val| {
            if let Some(val) = opt_val {
                Decimal::from_f64_retain(val)
            } else {
                None
            }
        })
        .collect();

    let hundred = Decimal::new(100, 0);

    let get_roc = |period: usize| -> Vec<Option<Decimal>> {
        let mut roc = vec![None; close_vec.len()];
        for i in period..close_vec.len() {
            if let (Some(curr), Some(prev)) = (close_vec[i], close_vec[i - period]) {
                if !prev.is_zero() {
                    roc[i] = Some(((curr - prev) / prev) * hundred);
                }
            }
        }
        roc
    };

    let get_sma = |roc_vals: &Vec<Option<Decimal>>, period: usize| -> Vec<Option<Decimal>> {
        let mut sma = vec![None; roc_vals.len()];
        let period_dec = Decimal::from_usize(period).unwrap_or(Decimal::ONE);

        let mut sum = Decimal::ZERO;
        let mut valid_count = 0;

        // Use a sliding window to maintain O(N) complexity
        for i in 0..roc_vals.len() {
            if let Some(val) = roc_vals[i] {
                sum += val;
                valid_count += 1;
            }

            // Remove the element that slid out of the window
            if i >= period {
                if let Some(old_val) = roc_vals[i - period] {
                    sum -= old_val;
                    valid_count -= 1;
                }
            }

            // Output value if window is fully valid
            if i >= period - 1 {
                if valid_count == period {
                    sma[i] = Some(sum / period_dec);
                }
            }
        }
        sma
    };

    let r1 = get_roc(roc1);
    let r2 = get_roc(roc2);
    let r3 = get_roc(roc3);
    let r4 = get_roc(roc4);

    let s1 = get_sma(&r1, sma1);
    let s2 = get_sma(&r2, sma2);
    let s3 = get_sma(&r3, sma3);
    let s4 = get_sma(&r4, sma4);

    let w1 = Decimal::ONE;
    let w2 = Decimal::new(2, 0);
    let w3 = Decimal::new(3, 0);
    let w4 = Decimal::new(4, 0);

    let mut kst_values: Vec<Option<f64>> = vec![None; close_vec.len()];

    for i in 0..close_vec.len() {
        if let (Some(v1), Some(v2), Some(v3), Some(v4)) = (s1[i], s2[i], s3[i], s4[i]) {
            let kst = (v1 * w1) + (v2 * w2) + (v3 * w3) + (v4 * w4);
            kst_values[i] = kst.to_f64();
        }
    }

    Ok(Series::new("kst", kst_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[
                10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0,
                20.0, 21.0, 22.0, 23.0, 24.0, 25.0, 26.0, 27.0, 28.0, 29.0
            ]
        )?;

        // Short periods for testing
        // r1=1, r2=2, r3=3, r4=4
        // s1=2, s2=2, s3=2, s4=2
        // max lookback = 4 (for r4) + 1 (for s4 minus 1) = 5
        let result = calculate(&df, 1, 2, 3, 4, 2, 2, 2, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 20);
        assert!(out.get(0).is_none());
        assert!(out.get(4).is_none());
        assert!(out.get(5).is_some());

        // Validate exact mathematical values calculated via known reference script
        // Expected value at index 5: ~270.6052
        // Expected value at index 10: ~186.2141
        // Expected value at index 19: ~119.3783

        let val5 = out.get(5).context("Expected value at index 5")?;
        assert!((val5 - 270.605228).abs() < 1e-4);

        let val10 = out.get(10).context("Expected value at index 10")?;
        assert!((val10 - 186.214095).abs() < 1e-4);

        let val19 = out.get(19).context("Expected value at index 19")?;
        assert!((val19 - 119.378306).abs() < 1e-4);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 10, 15, 20, 30, 10, 10, 10, 15);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0])?;
        let res_short = calculate(&df_short, 10, 15, 20, 30, 10, 10, 10, 15)?;
        let out_short = res_short.f64()?;
        assert_eq!(out_short.len(), 1);
        assert!(out_short.get(0).is_none());

        // Zero periods
        let df_normal = df!("close" => &[10.0, 11.0])?;
        assert!(calculate(&df_normal, 0, 15, 20, 30, 10, 10, 10, 15).is_err());
        assert!(calculate(&df_normal, 10, 15, 20, 30, 0, 10, 10, 15).is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let df = df!("close" => values)?;
        // Standard Pring periods: 10, 15, 20, 30 and 10, 10, 10, 15
        let result = calculate(&df, 10, 15, 20, 30, 10, 10, 10, 15)?;

        assert_eq!(result.len(), 100);
        let out = result.f64()?;

        // 30 (roc4) + 15 (sma4) - 1 = 44 index
        assert!(out.get(43).is_none());
        assert!(out.get(44).is_some());

        Ok(())
    }
}
