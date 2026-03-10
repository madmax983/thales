//! Double Exponential Moving Average (DEMA)
//!
//! Calculates the Double Exponential Moving Average, an indicator designed to reduce the lag of traditional exponential moving averages.
//!
//! DEMA = (2 * EMA) - EMA(EMA)

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use super::ema;

/// Calculate Double Exponential Moving Average (DEMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with DEMA values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::dema;
/// // let df = ...;
/// // let dema = dema::calculate(&df, 14)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Step 1: Calculate the first EMA
    let mut ema1_series = ema::calculate(data, period)?;

    // Create a new DataFrame with the "close" column containing the EMA1 values
    // to pass into the EMA function again.
    let ema1_df = DataFrame::new(vec![ema1_series.rename("close").clone()])
        .context("Failed to create temporary DataFrame for EMA1")?;

    // Step 2: Calculate the second EMA (EMA of EMA1)
    let ema2_series = ema::calculate(&ema1_df, period)?;

    // Get the underlying f64 values for the two series
    let ema1_f64 = ema1_series.f64().context("EMA1 must be numeric")?;
    let ema2_f64 = ema2_series.f64().context("EMA2 must be numeric")?;

    let mut dema_values: Vec<Option<f64>> = vec![None; ema1_f64.len()];

    // Step 3: DEMA = 2 * EMA1 - EMA2
    for i in 0..ema1_f64.len() {
        if let (Some(e1), Some(e2)) = (ema1_f64.get(i), ema2_f64.get(i)) {
            // Memory constraint: Use Decimal for all calculations (NO f64).
            // Convert to Decimal, perform math, then optionally convert back to f64 for storage.
            let e1_dec = Decimal::from_f64_retain(e1);
            let e2_dec = Decimal::from_f64_retain(e2);

            if let (Some(d1), Some(d2)) = (e1_dec, e2_dec) {
                let two = Decimal::new(2, 0);
                let dema_dec = (two * d1) - d2;
                dema_values[i] = Some(dema_dec.to_f64().unwrap_or(0.0));
            } else {
                dema_values[i] = None;
            }
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
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_normal = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());
        assert_eq!(
            res_zero.unwrap_err().to_string(),
            "Period must be greater than 0"
        );

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

    #[test]
    fn test_dema_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
        )?;

        // For Period 3:
        // EMA1:
        // 0: 10.0 -> None
        // 1: 11.0 -> None
        // 2: 12.0 -> SMA(10,11,12) = 11.0
        // 3: 13.0 -> 11.0 * 0.5 + 13.0 * 0.5 = 12.0
        // 4: 14.0 -> 12.0 * 0.5 + 14.0 * 0.5 = 13.0

        // EMA2 (EMA of EMA1):
        // 0: None -> None
        // 1: None -> None
        // 2: 11.0 -> None
        // 3: 12.0 -> None
        // 4: 13.0 -> SMA(11, 12, 13) = 12.0

        // DEMA = 2 * EMA1 - EMA2
        // 4: 2 * 13.0 - 12.0 = 26.0 - 12.0 = 14.0

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());
        assert!(out.get(3).is_none());
        assert_eq!(out.get(4), Some(14.0));

        Ok(())
    }
}
