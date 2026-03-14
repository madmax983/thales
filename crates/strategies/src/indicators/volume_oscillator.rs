//! Volume Oscillator
//!
//! Calculates the Volume Oscillator, a technical indicator that shows the difference
//! between two moving averages of volume, expressed as a percentage.
//! Positive values indicate short-term volume is higher than long-term volume (increasing volume trend).

use anyhow::{Context, Result};
use polars::prelude::*;

use super::sma;

/// Calculate Volume Oscillator
///
/// # Arguments
/// * `data` - DataFrame with "volume" column
/// * `short_period` - Short moving average period
/// * `long_period` - Long moving average period
///
/// # Returns
/// Series with Volume Oscillator values.
pub fn calculate(data: &DataFrame, short_period: usize, long_period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if short_period == 0 || long_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }
    if short_period >= long_period {
        anyhow::bail!("Short period must be less than long period");
    }

    let volume = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?
        .f64()
        .context("Volume column must be numeric (f64)")?;

    // We can use the existing SMA indicator for volume, by renaming volume to close temporarily.
    let mut vol_as_close = volume.clone().into_series();
    vol_as_close.rename("close");
    let sma_df = DataFrame::new(vec![vol_as_close])?;

    let short_sma = sma::calculate(&sma_df, short_period)?;
    let long_sma = sma::calculate(&sma_df, long_period)?;

    let short_sma_f64 = short_sma.f64()?;
    let long_sma_f64 = long_sma.f64()?;

    let mut vo_values: Vec<Option<f64>> = Vec::with_capacity(volume.len());

    for i in 0..volume.len() {
        if let (Some(s), Some(l)) = (short_sma_f64.get(i), long_sma_f64.get(i)) {
            if l == 0.0 {
                vo_values.push(Some(0.0));
            } else {
                let vo = ((s - l) / l) * 100.0;
                vo_values.push(Some(vo));
            }
        } else {
            vo_values.push(None);
        }
    }

    Ok(Series::new("volume_oscillator", vo_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_volume_oscillator() -> Result<()> {
        let df = df!(
            "volume" => &[100.0, 150.0, 200.0, 150.0, 100.0]
        )?;

        let result = calculate(&df, 2, 4)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 5);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());

        let val3 = out.get(3).unwrap();
        assert!((val3 - 16.666).abs() < 1e-2);

        Ok(())
    }

    #[test]
    fn test_edge_cases() {
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty, 2, 4).is_err());

        let df = df!("volume" => &[100.0, 200.0]).unwrap();
        assert!(calculate(&df, 4, 2).is_err()); // short >= long
    }
}
