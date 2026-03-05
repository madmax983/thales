use crate::indicators::ema;
use anyhow::{Context, Result};
use polars::prelude::*;

/// Calculate the True Strength Index (TSI)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `long_period` - First EMA smoothing period (typically 25)
/// * `short_period` - Second EMA smoothing period (typically 13)
///
/// # Returns
/// Series with TSI values ranging from -100 to +100.
pub fn calculate(data: &DataFrame, long_period: usize, short_period: usize) -> Result<Series> {
    if data.height() < 2 {
        anyhow::bail!("Data cannot be empty or have only one row");
    }
    if long_period == 0 || short_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

    let close_series = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    // 1. Calculate Momentum (price change) and Absolute Momentum
    let mut momentum_values: Vec<Option<f64>> = Vec::with_capacity(close_series.len());
    let mut abs_momentum_values: Vec<Option<f64>> = Vec::with_capacity(close_series.len());

    momentum_values.push(None); // First element has no previous
    abs_momentum_values.push(None);

    for i in 1..close_series.len() {
        if let (Some(current), Some(prev)) = (close_series.get(i), close_series.get(i - 1)) {
            let change = current - prev;
            momentum_values.push(Some(change));
            abs_momentum_values.push(Some(change.abs()));
        } else {
            momentum_values.push(None);
            abs_momentum_values.push(None);
        }
    }

    let pc_series = Series::new("close", momentum_values);
    let abs_pc_series = Series::new("close", abs_momentum_values);

    let pc_df = DataFrame::new(vec![pc_series.clone()])?;
    let abs_pc_df = DataFrame::new(vec![abs_pc_series.clone()])?;

    // 2. First smoothing (EMA of long_period)
    let pc_ema1 = ema::calculate(&pc_df, long_period)?;
    let abs_pc_ema1 = ema::calculate(&abs_pc_df, long_period)?;

    let pc_ema1_df = DataFrame::new(vec![pc_ema1.with_name("close").clone()])?;
    let abs_pc_ema1_df = DataFrame::new(vec![abs_pc_ema1.with_name("close").clone()])?;

    // 3. Second smoothing (EMA of short_period)
    let pc_ema2 = ema::calculate(&pc_ema1_df, short_period)?;
    let abs_pc_ema2 = ema::calculate(&abs_pc_ema1_df, short_period)?;

    // 4. Calculate TSI = 100 * (double smoothed PC / double smoothed absolute PC)
    let mut tsi_values: Vec<Option<f64>> = Vec::with_capacity(close_series.len());

    let pc_ema2_f64 = pc_ema2.f64()?;
    let abs_pc_ema2_f64 = abs_pc_ema2.f64()?;

    for i in 0..close_series.len() {
        if let (Some(num), Some(den)) = (pc_ema2_f64.get(i), abs_pc_ema2_f64.get(i)) {
            if den == 0.0 {
                tsi_values.push(Some(0.0));
            } else {
                let tsi = (num / den) * 100.0;
                tsi_values.push(Some(tsi));
            }
        } else {
            tsi_values.push(None);
        }
    }

    Ok(Series::new("tsi", tsi_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_tsi_calculation() -> Result<()> {
        let df = df!(
            "close" => &[
                100.0, 102.0, 101.0, 103.0, 105.0, 104.0, 106.0, 108.0, 107.0, 109.0,
                110.0, 108.0, 107.0, 106.0, 105.0, 104.0, 103.0, 102.0, 101.0, 100.0
            ]
        )?;

        // Short periods for testing
        let result = calculate(&df, 3, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 20);
        // First few should be None due to EMA ramp-up (1 for PC + 3 for EMA1 + 2 for EMA2 = needs several points)

        // Let's just verify it runs and produces reasonable bounds
        for i in 0..out.len() {
            if let Some(val) = out.get(i) {
                assert!((-100.0..=100.0).contains(&val));
            }
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 25, 13);
        assert!(res_empty.is_err());
        assert!(res_empty
            .unwrap_err()
            .to_string()
            .contains("Data cannot be empty"));

        // Single row
        let df_single = df!("close" => &[100.0])?;
        let res_single = calculate(&df_single, 25, 13);
        assert!(res_single.is_err());
        assert!(res_single
            .unwrap_err()
            .to_string()
            .contains("Data cannot be empty"));

        // Period > Data length should just return Nones
        let df_short = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_short = calculate(&df_short, 25, 13)?;
        assert_eq!(res_short.len(), 3);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());
        assert!(res_short.f64()?.get(2).is_none());

        Ok(())
    }
}
