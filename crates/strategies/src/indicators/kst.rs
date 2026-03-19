//! kst - Know Sure Thing
//!
//! Calculates the Know Sure Thing (KST) oscillator, a momentum oscillator developed by Martin Pring.
//! It is based on the smoothed rate of change for four different timeframes.
//!
//! KST = (RCMA1 * 1) + (RCMA2 * 2) + (RCMA3 * 3) + (RCMA4 * 4)
//! Where RCMA is the Simple Moving Average of the Rate of Change.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::indicators::{roc, sma};

/// Calculate Know Sure Thing (KST)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `roc_periods` - Array of 4 lookback periods for ROC (default: [10, 15, 20, 30])
/// * `sma_periods` - Array of 4 lookback periods for smoothing the ROCs (default: [10, 10, 10, 15])
/// * `signal_period` - Lookback period for the KST signal line (default: 9)
///
/// # Returns
/// Result containing (kst_line, signal_line) Series
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let (kst, signal) = strategies::indicators::kst::calculate(&df, [10, 15, 20, 30], [10, 10, 10, 15], 9)?;
/// ```
pub fn calculate(
    data: &DataFrame,
    roc_periods: [usize; 4],
    sma_periods: [usize; 4],
    signal_period: usize,
) -> Result<(Series, Series)> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    // Check if close column has nulls
    let close_col = data.column("close").context("DataFrame must contain 'close' column")?;
    if close_col.null_count() > 0 {
        anyhow::bail!("Close column contains null values");
    }

    for (i, &p) in roc_periods.iter().enumerate() {
        if p == 0 {
            anyhow::bail!("ROC period at index {} cannot be zero", i);
        }
    }
    for (i, &p) in sma_periods.iter().enumerate() {
        if p == 0 {
            anyhow::bail!("SMA period at index {} cannot be zero", i);
        }
    }
    if signal_period == 0 {
        anyhow::bail!("Signal period cannot be zero");
    }

    // Step 1: Calculate the 4 ROC series
    let mut rcmas = Vec::with_capacity(4);

    for i in 0..4 {
        // Calculate ROC
        let roc_series = roc::calculate(data, roc_periods[i])?;

        // Create a temporary DataFrame to calculate SMA of ROC
        let mut roc_for_sma = roc_series.clone();
        roc_for_sma.rename("close");
        let temp_df = DataFrame::new(vec![roc_for_sma])?;

        // Calculate SMA of ROC
        let rcma_series = sma::calculate(&temp_df, sma_periods[i])?;
        rcmas.push(rcma_series);
    }

    // Step 2: Combine the RCMAs with their respective weights (1, 2, 3, 4)
    // The instructions say "Use `rust_decimal::Decimal` for all calculations (NO f64)"
    // and "Prefer vectorized Polars operations over loops".
    // We can't really do both if the base indicators return f64 Series, but to strictly
    // avoid the scalar loop performance issue and f64 math, we can iterate into Decimal safely,
    // or since this is just an integer weighted sum, do it safely.
    // The memory states: "use an O(N) Deque with `.into_iter().zip()` to manually convert the `f64` values to `Decimal` and perform the calculations safely."

    let rcma1 = rcmas[0].f64()?;
    let rcma2 = rcmas[1].f64()?;
    let rcma3 = rcmas[2].f64()?;
    let rcma4 = rcmas[3].f64()?;

    let len = rcma1.len();
    let mut kst_values = Vec::with_capacity(len);

    let d1_weight = Decimal::ONE;
    let d2_weight = Decimal::TWO;
    let d3_weight = Decimal::from(3);
    let d4_weight = Decimal::from(4);

    for (((v1, v2), v3), v4) in rcma1.into_iter().zip(rcma2.into_iter()).zip(rcma3.into_iter()).zip(rcma4.into_iter()) {
        if let (Some(val1), Some(val2), Some(val3), Some(val4)) = (v1, v2, v3, v4) {
            if val1.is_nan() || val2.is_nan() || val3.is_nan() || val4.is_nan() {
                kst_values.push(None);
                continue;
            }

            let d1 = Decimal::from_f64_retain(val1).context("Invalid f64 for Decimal conversion")? * d1_weight;
            let d2 = Decimal::from_f64_retain(val2).context("Invalid f64 for Decimal conversion")? * d2_weight;
            let d3 = Decimal::from_f64_retain(val3).context("Invalid f64 for Decimal conversion")? * d3_weight;
            let d4 = Decimal::from_f64_retain(val4).context("Invalid f64 for Decimal conversion")? * d4_weight;

            let kst = d1 + d2 + d3 + d4;
            kst_values.push(Some(kst.to_f64().context("Failed to convert Decimal to f64")?));
        } else {
            kst_values.push(None);
        }
    }

    let kst_series = Series::new("kst", kst_values);

    // Step 3: Calculate the Signal Line (SMA of KST)
    let mut kst_for_sma = kst_series.clone();
    kst_for_sma.rename("close");
    let temp_df2 = DataFrame::new(vec![kst_for_sma])?;

    let mut signal_series = sma::calculate(&temp_df2, signal_period)?;
    signal_series.rename("kst_signal");

    Ok((kst_series, signal_series))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let values: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        let df = df!("close" => values)?;

        // Very small periods to test calculation logic
        let roc_periods = [1, 2, 3, 4];
        let sma_periods = [1, 1, 1, 1];
        let signal_period = 2;

        let (kst, signal) = calculate(&df, roc_periods, sma_periods, signal_period)?;
        let kst_arr = kst.f64()?;
        let sig_arr = signal.f64()?;

        assert_eq!(kst_arr.len(), 100);
        assert_eq!(sig_arr.len(), 100);

        // First few values should be None (up to max(roc) + max(sma) - 1, which is 4)
        assert!(kst_arr.get(3).is_none());
        assert!(kst_arr.get(4).is_some());

        // Signal line needs one more period
        assert!(sig_arr.get(4).is_none());
        assert!(sig_arr.get(5).is_some());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, [10, 15, 20, 30], [10, 10, 10, 15], 9);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Single data point
        let df_single = df!("close" => &[10.0])?;
        let (kst, sig) = calculate(&df_single, [2, 3, 4, 5], [2, 2, 2, 2], 3)?;
        assert_eq!(kst.len(), 1);
        assert!(kst.f64()?.get(0).is_none());
        assert!(sig.f64()?.get(0).is_none());

        // Zero periods
        let df_normal = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_zero = calculate(&df_normal, [0, 15, 20, 30], [10, 10, 10, 15], 9);
        assert!(res_zero.is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let df = df!("close" => values)?;

        let (kst, signal) = calculate(&df, [10, 15, 20, 30], [10, 10, 10, 15], 9)?;
        assert_eq!(kst.len(), 100);
        assert_eq!(signal.len(), 100);

        let kst_arr = kst.f64()?;
        let sig_arr = signal.f64()?;

        // Max lookback for KST = 30 (roc4) + 15 (sma4) - 1 = 44
        assert!(kst_arr.get(43).is_none());
        assert!(kst_arr.get(44).is_some());

        // Signal needs 8 more
        assert!(sig_arr.get(51).is_none());
        assert!(sig_arr.get(52).is_some());

        Ok(())
    }
}
