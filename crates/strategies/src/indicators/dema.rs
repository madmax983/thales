//! DEMA - Double Exponential Moving Average
//!
//! Calculates the Double Exponential Moving Average, an indicator designed to reduce the lag of traditional exponential moving averages.
//!
//! DEMA = 2 * EMA(price, period) - EMA(EMA(price, period), period)

use crate::indicators::ema;
use anyhow::Result;
use polars::prelude::*;

/// Calculate Double Exponential Moving Average (DEMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with DEMA values. The first `period * 2 - 1` values will be null.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Step 1: Calculate first EMA
    let ema1_series = ema::calculate(data, period)?;

    // Step 2: Create a new DataFrame with ema1 as the "close" column
    // This is required because ema::calculate expects a "close" column
    let ema1_df = DataFrame::new(vec![ema1_series.clone().with_name("close")])?;

    // Step 3: Calculate second EMA (EMA of EMA1)
    let ema2_series = ema::calculate(&ema1_df, period)?;

    // Step 4: Calculate DEMA = 2 * EMA1 - EMA2
    let ema1_f64 = ema1_series.f64()?;
    let ema2_f64 = ema2_series.f64()?;

    let mut dema_values: Vec<Option<f64>> = Vec::with_capacity(ema1_f64.len());

    for i in 0..ema1_f64.len() {
        if let (Some(e1), Some(e2)) = (ema1_f64.get(i), ema2_f64.get(i)) {
            dema_values.push(Some(2.0 * e1 - e2));
        } else {
            dema_values.push(None);
        }
    }

    Ok(Series::new("dema", dema_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_dema_basic() -> Result<()> {
        // Generating some dummy data
        let values: Vec<f64> = (1..=20).map(|v| v as f64).collect();
        let df = df!("close" => values)?;

        let period = 3;
        let dema_series = calculate(&df, period)?;
        let dema = dema_series.f64()?;

        assert_eq!(dema.len(), 20);

        // Ensure initial values are null due to EMA warmup
        assert!(dema.get(0).is_none());
        assert!(dema.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.f64()?;
        assert_eq!(out.len(), 2);
        assert!(out.get(0).is_none());

        Ok(())
    }
}
