use anyhow::Result;
use polars::prelude::*;

use crate::indicators::ema;

/// Calculate Triple Exponential Moving Average (TEMA)
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

    // EMA1
    let ema1_series = ema::calculate(data, period)?;

    // To calculate EMA2, we need a DataFrame with EMA1 as "close"
    let ema1_df = DataFrame::new(vec![ema1_series.clone().with_name("close")])?;
    let ema2_series = ema::calculate(&ema1_df, period)?;

    // To calculate EMA3, we need a DataFrame with EMA2 as "close"
    let ema2_df = DataFrame::new(vec![ema2_series.clone().with_name("close")])?;
    let ema3_series = ema::calculate(&ema2_df, period)?;

    let ema1 = ema1_series.f64()?;
    let ema2 = ema2_series.f64()?;
    let ema3 = ema3_series.f64()?;

    let mut tema_values: Vec<Option<f64>> = Vec::with_capacity(ema1.len());

    for i in 0..ema1.len() {
        if let (Some(e1), Some(e2), Some(e3)) = (ema1.get(i), ema2.get(i), ema3.get(i)) {
            let tema = (3.0 * e1) - (3.0 * e2) + e3;
            tema_values.push(Some(tema));
        } else {
            tema_values.push(None);
        }
    }

    Ok(Series::new("tema", tema_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_tema_calculation() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 10);
        // Initial values will be None due to EMA seeding
        assert!(out.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }
}
