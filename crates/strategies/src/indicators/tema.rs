use anyhow::Result;
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

    // 1. Calculate EMA1
    let ema1 = ema::calculate(data, period)?;

    // Create a new DataFrame using EMA1 as the "close" column to calculate EMA2
    let ema1_df = DataFrame::new(vec![ema1.clone().with_name("close")])?;
    let ema2 = ema::calculate(&ema1_df, period)?;

    // Create a new DataFrame using EMA2 as the "close" column to calculate EMA3
    let ema2_df = DataFrame::new(vec![ema2.clone().with_name("close")])?;
    let ema3 = ema::calculate(&ema2_df, period)?;

    // TEMA = (3 * EMA1) - (3 * EMA2) + EMA3
    let ema1_x_3 = &ema1 * 3.0;
    let ema2_x_3 = &ema2 * 3.0;

    let diff = (&ema1_x_3 - &ema2_x_3)?;
    let mut tema = (&diff + &ema3)?;

    let _ = tema.rename("tema");
    Ok(tema)
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

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        // Output len should be 10
        assert_eq!(out.len(), 10);

        // Initial values will be None due to EMA seeding
        assert!(out.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        } else {
            panic!("Expected error for empty data");
        }

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }
}
