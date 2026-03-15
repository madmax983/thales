use anyhow::{Context, Result};
use polars::prelude::*;

use super::ema;

/// Calculate Triple Exponential Moving Average (TEMA)
///
/// TEMA = (3 * EMA1) - (3 * EMA2) + EMA3
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with TEMA values.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let mut ema1_series = ema::calculate(data, period)?;
    let ema1_df = DataFrame::new(vec![ema1_series.rename("close").clone()])
        .context("Failed to create temporary DataFrame for EMA1")?;

    let mut ema2_series = ema::calculate(&ema1_df, period)?;
    let ema2_df = DataFrame::new(vec![ema2_series.rename("close").clone()])
        .context("Failed to create temporary DataFrame for EMA2")?;

    let ema3_series = ema::calculate(&ema2_df, period)?;

    let ema1_f64 = ema1_series.f64().context("EMA1 must be numeric")?;
    let ema2_f64 = ema2_series.f64().context("EMA2 must be numeric")?;
    let ema3_f64 = ema3_series.f64().context("EMA3 must be numeric")?;

    let mut tema_values: Vec<Option<f64>> = vec![None; ema1_f64.len()];

    for (i, val) in tema_values.iter_mut().enumerate().take(ema1_f64.len()) {
        if let (Some(e1), Some(e2), Some(e3)) = (ema1_f64.get(i), ema2_f64.get(i), ema3_f64.get(i))
        {
            let tema = (3.0 * e1) - (3.0 * e2) + e3;
            *val = Some(tema);
        }
    }

    Ok(Series::new("tema", tema_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_tema_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());

        let df_normal = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());
        Ok(())
    }

    #[test]
    fn test_tema_calculation() -> Result<()> {
        let values: Vec<f64> = (0..100).map(|i| 10.0 + (i as f64 * 0.1)).collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 5)?;
        let s = result.f64()?;

        assert_eq!(s.len(), 100);
        assert!(s.get(99).is_some());

        Ok(())
    }
}
