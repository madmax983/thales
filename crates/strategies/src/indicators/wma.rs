//! WMA - Weighted Moving Average
//!
//! Calculates the Weighted Moving Average (WMA), which places a greater weight on more recent data points.
//! The weights decrease linearly from the most recent to the oldest data point in the window.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Weighted Moving Average (WMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period (must be > 0)
///
/// # Returns
/// Series with WMA values. The first `period - 1` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let wma = strategies::indicators::wma::calculate(&df, 14)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let mut wma_values: Vec<Option<f64>> = vec![None; close.len()];

    if close.len() < period {
        return Ok(Series::new("wma", wma_values));
    }

    // Calculate sum of weights: n + (n-1) + ... + 1 = n * (n + 1) / 2
    let sum_of_weights = Decimal::from_usize(period * (period + 1) / 2)
        .context("Failed to create decimal from period")?;

    for (i, wma_val) in wma_values
        .iter_mut()
        .enumerate()
        .take(close.len())
        .skip(period - 1)
    {
        let mut sum = Decimal::ZERO;
        let mut valid_window = true;

        for j in 0..period {
            let idx = i - j;
            let weight =
                Decimal::from_usize(period - j).context("Failed to create decimal from weight")?;

            if let Some(price_f64) = close.get(idx) {
                let price = Decimal::from_f64_retain(price_f64).unwrap_or(Decimal::ZERO);
                sum += price * weight;
            } else {
                valid_window = false;
                break;
            }
        }

        if valid_window {
            let wma = sum / sum_of_weights;
            *wma_val = Some(wma.to_f64().unwrap_or(0.0));
        }
    }

    Ok(Series::new("wma", wma_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        // Test with known reference values
        // WMA(3)
        // Data: [10.0, 11.0, 12.0, 13.0]
        // Weights for period 3: 3, 2, 1. Sum of weights = 6.
        // Index 0: None
        // Index 1: None
        // Index 2 (10, 11, 12): (12*3 + 11*2 + 10*1) / 6 = (36 + 22 + 10) / 6 = 68 / 6 = 11.333333
        // Index 3 (11, 12, 13): (13*3 + 12*2 + 11*1) / 6 = (39 + 24 + 11) / 6 = 74 / 6 = 12.333333

        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 4);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).unwrap();
        assert!(
            (val2 - 11.333333).abs() < 1e-5,
            "Expected ~11.333333, got {}",
            val2
        );

        let val3 = out.get(3).unwrap();
        assert!(
            (val3 - 12.333333).abs() < 1e-5,
            "Expected ~12.333333, got {}",
            val3
        );

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.f64()?;
        assert_eq!(out.len(), 3);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());

        // Zero period
        let df_normal = df!("close" => &[10.0, 11.0])?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());
        assert_eq!(
            res_zero.unwrap_err().to_string(),
            "Period must be greater than 0"
        );

        // Missing data (nulls)
        let s_close = Series::new("close", &[Some(10.0), None, Some(12.0), Some(13.0)]);
        let df_nulls = DataFrame::new(vec![s_close])?;
        let res_nulls = calculate(&df_nulls, 2)?;
        let out_nulls = res_nulls.f64()?;

        assert_eq!(out_nulls.len(), 4);
        assert!(out_nulls.get(0).is_none()); // Not enough data
        assert!(out_nulls.get(1).is_none()); // Contains null
        assert!(out_nulls.get(2).is_none()); // Contains null (window is None, 12.0)

        let val3 = out_nulls.get(3).unwrap(); // window is 12.0, 13.0
        assert!(
            (val3 - 12.666666).abs() < 1e-5,
            "Expected ~12.666666, got {}",
            val3
        );

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 14);
        assert!(result.is_ok());
        let s = result?;
        assert_eq!(s.len(), 100);

        let out = s.f64()?;
        assert!(out.get(12).is_none());
        assert!(out.get(13).is_some());

        for i in 13..100 {
            assert!(out.get(i).is_some());
        }

        Ok(())
    }
}
