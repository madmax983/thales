//! SMMA - Smoothed Moving Average
//!
//! Calculates the Smoothed Moving Average (SMMA), which is a type of Exponential
//! Moving Average (EMA) with a different smoothing factor.
//!
//! # Implementation Details
//! - Uses `rust_decimal::Decimal` iteratively for all math calculations to ensure financial precision, explicitly converting from and to `f64` only for Polars boundaries.
//!
//! # Returns
//! Series with SMMA values.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use crate::indicators::sma;

/// Calculate Smoothed Moving Average (SMMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period for SMMA (e.g., 14)
///
/// # Returns
/// Series with SMMA values. First `period - 1` values will be null.
/// Series name: "smma"
///
/// # Example
/// ```rust
/// use strategies::indicators::smma;
/// use polars::prelude::*;
/// let df = df!("close" => &[10.0, 11.0, 12.0]).unwrap();
/// let result = smma::calculate(&df, 2).unwrap();
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be > 0");
    }

    let close_series = data.column("close").context("Missing 'close' column")?;
    let close_arr = close_series.f64()?;

    let mut smma_f64: Vec<Option<f64>> = vec![None; data.height()];

    if data.height() < period {
        let series = Series::new("smma", smma_f64);
        return Ok(series);
    }

    // Calculate initial SMA for the first valid value
    let sma_series = sma::calculate(data, period)?;
    let sma_arr = sma_series.f64()?;

    let dec_period = Decimal::from_usize(period).context("Failed to parse period to Decimal")?;
    let mut prev_smma: Option<Decimal> = None;

    for (i, val) in smma_f64.iter_mut().enumerate().take(data.height()) {
        if i < period - 1 {
            continue;
        }

        let price_opt = close_arr.get(i).and_then(Decimal::from_f64);

        if let Some(price) = price_opt {
            if let Some(prev) = prev_smma {
                // SMMA(i) = (SMMA(i-1) * (period - 1) + Price(i)) / period
                let smma_val = (prev * (dec_period - Decimal::ONE) + price) / dec_period;
                *val = smma_val.to_f64();
                prev_smma = Some(smma_val);
            } else if let Some(initial_sma) = sma_arr.get(i).and_then(Decimal::from_f64) {
                // First SMMA value is the SMA
                *val = initial_sma.to_f64();
                prev_smma = Some(initial_sma);
            }
        } else {
            prev_smma = None;
        }
    }

    let series = Series::new("smma", smma_f64);
    Ok(series)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_smma_calculation() -> Result<()> {
        let df = df!(
            "close" => &[ 10.0, 11.0, 12.0, 13.0, 14.0 ]
        )?;

        let smma = calculate(&df, 3)?;
        let smma_vals = smma.f64()?;

        // Index 0, 1 are null
        assert!(smma_vals.get(0).is_none());
        assert!(smma_vals.get(1).is_none());

        // Index 2 is SMA of [10.0, 11.0, 12.0] = 11.0
        let val2 = smma_vals.get(2).unwrap();
        assert!((val2 - 11.0).abs() < 0.001);

        // Index 3: (SMMA_prev * (3-1) + Price) / 3 = (11.0 * 2 + 13.0) / 3 = 35 / 3 = 11.666...
        let val3 = smma_vals.get(3).unwrap();
        assert!((val3 - 11.666).abs() < 0.005);

        // Index 4: (11.666... * 2 + 14.0) / 3 = (23.333... + 14.0) / 3 = 37.333... / 3 = 12.444...
        let val4 = smma_vals.get(4).unwrap();
        assert!((val4 - 12.444).abs() < 0.005);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());

        let df_single = df!("close" => &[ 10.0 ])?;
        let smma_single = calculate(&df_single, 14)?;
        assert_eq!(smma_single.len(), 1);
        assert!(smma_single.f64()?.get(0).is_none());

        let df_zero_period = df!("close" => &[ 10.0, 11.0 ])?;
        let res_zero = calculate(&df_zero_period, 0);
        assert!(res_zero.is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        // Mocking a volatile week
        let df = df!(
            "close" => &[
                150.25, 152.10, 149.80, 148.50, 151.30, // Week 1
                155.00, 158.20, 157.10, 154.30, 153.90, // Week 2
            ]
        )?;

        let period = 5;
        let smma = calculate(&df, period)?;
        let smma_vals = smma.f64()?;

        assert_eq!(smma_vals.len(), 10);

        // First 4 should be none
        for i in 0..4 {
            assert!(smma_vals.get(i).is_none());
        }

        // Index 4 is SMA
        let sma_val = (150.25 + 152.10 + 149.80 + 148.50 + 151.30) / 5.0; // 150.39
        let val4 = smma_vals.get(4).unwrap();
        assert!((val4 - sma_val).abs() < 0.01);

        // Index 5: (150.39 * 4 + 155.00) / 5 = 151.312
        let smma_5_expected = (sma_val * 4.0 + 155.00) / 5.0;
        let val5 = smma_vals.get(5).unwrap();
        assert!((val5 - smma_5_expected).abs() < 0.01);

        Ok(())
    }
}
