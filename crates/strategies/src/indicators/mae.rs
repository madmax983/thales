//! Moving Average Envelopes (MAE)
//!
//! Moving Average Envelopes consist of a moving average (typically SMA or EMA)
//! and two envelopes (upper and lower) set at a percentage distance from the moving average.
//!
//! # Returns
//! Tuple of (Series upper_band, Series lower_band, Series ma).

use anyhow::Result;
use polars::prelude::*;

use crate::indicators::sma;

/// Calculate Moving Average Envelopes (SMA-based)
///
/// # Arguments
/// * `data` - DataFrame containing a "close" column
/// * `period` - The lookback period for the Simple Moving Average (SMA). (e.g., 20)
/// * `percentage` - The percentage distance for the envelopes (e.g., 2.5 for 2.5%)
///
/// # Returns
/// Tuple of (Series upper_band, Series lower_band, Series sma).
pub fn calculate(data: &DataFrame, period: usize, percentage: f64) -> Result<(Series, Series, Series)> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    let sma_series = sma::calculate(data, period)?;
    let sma_arr = sma_series.f64()?;

    let mut upper_f64: Vec<Option<f64>> = vec![None; data.height()];
    let mut lower_f64: Vec<Option<f64>> = vec![None; data.height()];

    let factor = percentage / 100.0;

    for i in 0..data.height() {
        if let Some(ma_val) = sma_arr.get(i) {
            let offset = ma_val * factor;
            upper_f64[i] = Some(ma_val + offset);
            lower_f64[i] = Some(ma_val - offset);
        }
    }

    let mut sma_series_renamed = sma_series.clone();
    sma_series_renamed.rename("mae_sma");

    let upper_series = Series::new("mae_upper", upper_f64);
    let lower_series = Series::new("mae_lower", lower_f64);

    Ok((upper_series, lower_series, sma_series_renamed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_mae_calculation() -> Result<()> {
        let df = df!(
            "close" => &[ 100.0, 100.0, 100.0, 100.0, 100.0 ]
        )?;

        let (upper, lower, sma) = calculate(&df, 2, 5.0)?;
        let upper_vals = upper.f64()?;
        let lower_vals = lower.f64()?;
        let sma_vals = sma.f64()?;

        let upper_val1 = upper_vals.get(1).unwrap();
        let lower_val1 = lower_vals.get(1).unwrap();
        let sma_val1 = sma_vals.get(1).unwrap();

        assert!((sma_val1 - 100.0).abs() < 0.001);
        assert!((upper_val1 - 105.0).abs() < 0.001);
        assert!((lower_val1 - 95.0).abs() < 0.001);

        Ok(())
    }

    #[test]
    fn test_empty_data() {
        let df_empty = DataFrame::default();
        let res = calculate(&df_empty, 10, 2.5);
        assert!(res.is_err());
    }
}
