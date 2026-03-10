//! TEMA - Triple Exponential Moving Average

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use crate::indicators::ema;

/// Calculate Triple Exponential Moving Average (TEMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with TEMA values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::tema;
///
/// let df = df!("close" => &[10.0, 11.0, 12.0, 13.0, 14.0]).unwrap();
/// let result = tema::calculate(&df, 3).unwrap();
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Calculate EMA1
    let mut ema1_series = ema::calculate(data, period)?;

    // Prepare DataFrame for EMA2
    let ema1_df = DataFrame::new(vec![ema1_series.rename("close").clone()])
        .context("Failed to create temporary DataFrame for EMA1")?;

    // Calculate EMA2
    let mut ema2_series = ema::calculate(&ema1_df, period)?;

    // Prepare DataFrame for EMA3
    let ema2_df = DataFrame::new(vec![ema2_series.rename("close").clone()])
        .context("Failed to create temporary DataFrame for EMA2")?;

    // Calculate EMA3
    let ema3_series = ema::calculate(&ema2_df, period)?;

    let ema1_f64 = ema1_series.f64().context("EMA1 must be numeric")?;
    let ema2_f64 = ema2_series.f64().context("EMA2 must be numeric")?;
    let ema3_f64 = ema3_series.f64().context("EMA3 must be numeric")?;

    let mut tema_values: Vec<Option<f64>> = Vec::with_capacity(ema1_f64.len());

    let three = Decimal::from(3);

    for i in 0..ema1_f64.len() {
        let e1_opt = ema1_f64.get(i);
        let e2_opt = ema2_f64.get(i);
        let e3_opt = ema3_f64.get(i);

        if let (Some(e1), Some(e2), Some(e3)) = (e1_opt, e2_opt, e3_opt) {
            let e1_dec_opt = Decimal::from_f64_retain(e1);
            let e2_dec_opt = Decimal::from_f64_retain(e2);
            let e3_dec_opt = Decimal::from_f64_retain(e3);

            if let (Some(e1_dec), Some(e2_dec), Some(e3_dec)) = (e1_dec_opt, e2_dec_opt, e3_dec_opt) {
                // TEMA = (3 * EMA1) - (3 * EMA2) + EMA3
                let tema = (three * e1_dec) - (three * e2_dec) + e3_dec;
                tema_values.push(tema.to_f64());
            } else {
                tema_values.push(None);
            }
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
    fn test_known_values() -> Result<()> {
        // Calculate TEMA manually for small set to verify logic
        let df = df!("close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0])?;
        let period = 2;
        let result = calculate(&df, period)?;
        let s = result.f64()?;

        // TEMA should be mostly None at the start due to nested EMAs
        assert!(s.get(0).is_none());
        assert!(s.get(1).is_none());
        assert!(s.get(2).is_none());

        // By end of the sequence, it should have a value and follow the trend
        if let Some(val) = s.get(5) {
            assert!(val > 14.0 && val < 16.0, "TEMA value should reflect the trend of values.");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        // Period > Data length
        let df_short = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.f64()?;
        assert_eq!(out.len(), 3);
        assert!(out.get(0).is_none());
        assert!(out.get(2).is_none());

        // Zero period
        let df_normal = df!("close" => &[10.0, 11.0])?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        // Create an upward trend
        let values: Vec<f64> = (0..100).map(|i| 10.0 + (i as f64 * 0.1)).collect();
        let df = df!("close" => values)?;

        // TEMA with period 10
        let result = calculate(&df, 10);
        assert!(result.is_ok());
        let s = result?;
        assert_eq!(s.len(), 100);

        let series = s.f64()?;
        let mut last_valid_idx = 0;

        for i in 0..100 {
            if series.get(i).is_some() {
                last_valid_idx = i;
                break;
            }
        }

        // Ensure there are some valid values at the end
        assert!(last_valid_idx > 0 && last_valid_idx < 99);
        assert!(series.get(99).is_some());

        Ok(())
    }
}
