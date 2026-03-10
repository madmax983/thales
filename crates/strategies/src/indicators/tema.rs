use anyhow::Result;
use polars::prelude::*;

use crate::indicators::ema;

/// Calculate Triple Exponential Moving Average (TEMA)
///
/// TEMA = (3 * EMA1) - (3 * EMA2) + EMA3
/// where:
/// EMA1 = EMA(Price)
/// EMA2 = EMA(EMA1)
/// EMA3 = EMA(EMA2)
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

    // EMA1
    let mut ema1_series = ema::calculate(data, period)?;

    // Create a DataFrame for EMA1 to pass to EMA function
    let ema1_df = DataFrame::new(vec![ema1_series.rename("close").clone()])?;

    // EMA2
    let mut ema2_series = ema::calculate(&ema1_df, period)?;

    // Create a DataFrame for EMA2 to pass to EMA function
    let ema2_df = DataFrame::new(vec![ema2_series.rename("close").clone()])?;

    // EMA3
    let ema3_series = ema::calculate(&ema2_df, period)?;

    let ema1_arr = ema1_series.f64()?;
    let ema2_arr = ema2_series.f64()?;
    let ema3_arr = ema3_series.f64()?;

    let mut tema_values: Vec<Option<f64>> = Vec::with_capacity(ema1_arr.len());

    for i in 0..ema1_arr.len() {
        if let (Some(e1), Some(e2), Some(e3)) = (ema1_arr.get(i), ema2_arr.get(i), ema3_arr.get(i)) {
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
        // Need a long enough series to calculate EMA3.
        // EMA1 requires `period` bars.
        // EMA2 requires `period` bars of EMA1 (so 2 * period - 1 total bars)
        // EMA3 requires `period` bars of EMA2 (so 3 * period - 2 total bars)
        // Let's use period = 3, so we need 3 * 3 - 2 = 7 bars for the first TEMA value.
        let mut closes = Vec::new();
        for i in 1..=20 {
            closes.push(10.0 + i as f64);
        }

        let df = df!(
            "close" => closes
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        // Ensure the correct length
        assert_eq!(out.len(), 20);

        // The first 6 bars should be None (indexes 0 to 5)
        for i in 0..6 {
            assert!(out.get(i).is_none(), "Expected None at index {}", i);
        }

        // The 7th bar (index 6) should have a value
        assert!(out.get(6).is_some(), "Expected Some at index 6");

        Ok(())
    }

    #[test]
    fn test_edge_cases() {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!("close" => &[10.0, 11.0]).unwrap();
        let res_short = calculate(&df_short, 5).unwrap();
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64().unwrap().get(0).is_none());
        assert!(res_short.f64().unwrap().get(1).is_none());
    }
}
