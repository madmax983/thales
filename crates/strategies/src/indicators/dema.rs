//! Double Exponential Moving Average (DEMA)
//!
//! Calculates the DEMA indicator, which reduces lag compared to traditional EMAs.
//! DEMA = (2 * EMA) - EMA(EMA)

use anyhow::{Context, Result};
use polars::prelude::*;

use super::ema;

/// Calculate Double Exponential Moving Average (DEMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with DEMA values.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // 1. Calculate Single EMA
    let mut ema1_series = ema::calculate(data, period)?;

    // Create temporary DataFrame for EMA2 calculation
    let ema1_df = DataFrame::new(vec![ema1_series.rename("close").clone()])
        .context("Failed to create temporary DataFrame for EMA1")?;

    // 2. Calculate Double EMA (EMA of EMA)
    let ema2_series = ema::calculate(&ema1_df, period)?;

    let ema1_f64 = ema1_series.f64().context("EMA1 must be numeric")?;
    let ema2_f64 = ema2_series.f64().context("EMA2 must be numeric")?;

    let mut dema_values: Vec<Option<f64>> = vec![None; ema1_f64.len()];

    for (i, val) in dema_values.iter_mut().enumerate().take(ema1_f64.len()) {
        if let (Some(e1), Some(e2)) = (ema1_f64.get(i), ema2_f64.get(i)) {
            let dema = (2.0 * e1) - e2;
            *val = Some(dema);
        }
    }

    Ok(Series::new("dema", dema_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_dema_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());

        let df_normal = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());
        Ok(())
    }

    #[test]
    fn test_dema_calculation() -> Result<()> {
        let values: Vec<f64> = (0..100).map(|i| 10.0 + (i as f64 * 0.1)).collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 5)?;
        let s = result.f64()?;

        assert_eq!(s.len(), 100);
        assert!(s.get(99).is_some());

        Ok(())
    }
}
