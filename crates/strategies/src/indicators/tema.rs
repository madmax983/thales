use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::indicators::ema;

/// Calculate Triple Exponential Moving Average (TEMA)
/// Formula: TEMA = (3 * EMA1) - (3 * EMA2) + EMA3
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with TEMA values.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // 1. Calculate EMA1
    let ema1_series = ema::calculate(data, period)?;

    // We need to create a new DataFrame to calculate EMA2 from EMA1
    // The ema::calculate function expects a "close" column.
    let df_ema1 = DataFrame::new(vec![ema1_series.clone().with_name("close".into())])?;
    let ema2_series = ema::calculate(&df_ema1, period)?;

    // Calculate EMA3 from EMA2
    let df_ema2 = DataFrame::new(vec![ema2_series.clone().with_name("close".into())])?;
    let ema3_series = ema::calculate(&df_ema2, period)?;

    // Get the arrays
    let ema1_arr = ema1_series.f64()?;
    let ema2_arr = ema2_series.f64()?;
    let ema3_arr = ema3_series.f64()?;

    let mut tema_values: Vec<Option<f64>> = Vec::with_capacity(ema1_arr.len());

    let three = Decimal::new(3, 0);

    for i in 0..ema1_arr.len() {
        let e1 = ema1_arr.get(i).and_then(Decimal::from_f64_retain);
        let e2 = ema2_arr.get(i).and_then(Decimal::from_f64_retain);
        let e3 = ema3_arr.get(i).and_then(Decimal::from_f64_retain);

        if let (Some(v1), Some(v2), Some(v3)) = (e1, e2, e3) {
            // TEMA = (3 * EMA1) - (3 * EMA2) + EMA3
            let tema = (three * v1) - (three * v2) + v3;
            tema_values.push(tema.to_f64());
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
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0]
        )?;

        // Period 2.
        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 9);
        // It should have None at the beginning because EMA calculations need seed
        assert!(out.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }
}
