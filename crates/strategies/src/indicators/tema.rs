use crate::indicators::ema;
use anyhow::{Context, Result};
use polars::prelude::*;

/// Calculate Triple Exponential Moving Average (TEMA)
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

    // TEMA = (3 * EMA1) - (3 * EMA2) + EMA3
    // EMA1 = EMA(price)
    // EMA2 = EMA(EMA1)
    // EMA3 = EMA(EMA2)

    let ema1 = ema::calculate(data, period)?;

    // Create a new dataframe with EMA1 to calculate EMA2
    let ema1_df = DataFrame::new(vec![ema1.clone().with_name("close")])
        .context("Failed to create DataFrame for EMA2")?;
    let ema2 = ema::calculate(&ema1_df, period)?;

    // Create a new dataframe with EMA2 to calculate EMA3
    let ema2_df = DataFrame::new(vec![ema2.clone().with_name("close")])
        .context("Failed to create DataFrame for EMA3")?;
    let ema3 = ema::calculate(&ema2_df, period)?;

    let ema1_f64 = ema1.f64()?;
    let ema2_f64 = ema2.f64()?;
    let ema3_f64 = ema3.f64()?;

    let len = ema1.len();
    let mut tema_values = Vec::with_capacity(len);

    for i in 0..len {
        let v1 = ema1_f64.get(i);
        let v2 = ema2_f64.get(i);
        let v3 = ema3_f64.get(i);

        if let (Some(val1), Some(val2), Some(val3)) = (v1, v2, v3) {
            let tema = (3.0 * val1) - (3.0 * val2) + val3;
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
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 9);
        // The first few values will be None because EMA needs a seed (SMA of period)
        // EMA1 needs 3, EMA2 needs 3 more, EMA3 needs 3 more.
        // We just check that it runs and produces output correctly.
        assert!(out.get(0).is_none());

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
        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }
}
