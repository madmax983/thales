//! DEMA - Double Exponential Moving Average
//!
//! Calculates the Double Exponential Moving Average (DEMA).
//! The DEMA is designed to be a faster moving average than the EMA,
//! reducing lag by adding the difference between the EMA and a smoothed EMA
//! back to the original EMA.
//!
//! Formula: DEMA = (2 * EMA) - EMA(EMA)

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
/// // let df = ...;
/// // let result = strategies::indicators::dema::calculate(&df, 14)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let mut ema1_series = ema::calculate(data, period)?;

    let ema1_df = DataFrame::new(vec![ema1_series.rename("close").clone()])
        .context("Failed to create temporary DataFrame for EMA1")?;

    let ema2_series = ema::calculate(&ema1_df, period)?;

    let ema1_f64 = ema1_series.f64().context("EMA1 must be numeric")?;
    let ema2_f64 = ema2_series.f64().context("EMA2 must be numeric")?;

    let mut dema_values: Vec<Option<f64>> = vec![None; ema1_f64.len()];
    let two = Decimal::new(2, 0);

    for i in 0..ema1_f64.len() {
        if let (Some(e1), Some(e2)) = (ema1_f64.get(i), ema2_f64.get(i)) {
            if let (Some(dec_e1), Some(dec_e2)) = (Decimal::from_f64_retain(e1), Decimal::from_f64_retain(e2)) {
                let dema = (two * dec_e1) - dec_e2;
                dema_values[i] = dema.to_f64();
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
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
        )?;

        // Period 2:
        // K = 2/3 ≈ 0.66666667
        // EMA1:
        //   idx 0: 10
        //   idx 1: SMA = 10.5
        //   idx 2: (12 * 2/3) + (10.5 * 1/3) = 8 + 3.5 = 11.5
        //   idx 3: (13 * 2/3) + (11.5 * 1/3) = 8.66666 + 3.83333 = 12.5
        //   idx 4: (14 * 2/3) + (12.5 * 1/3) = 9.33333 + 4.16666 = 13.5
        // EMA2 (EMA of EMA1):
        //   idx 0: None
        //   idx 1: 10.5
        //   idx 2: SMA(10.5, 11.5) = 11.0
        //   idx 3: (12.5 * 2/3) + (11.0 * 1/3) = 8.33333 + 3.66666 = 12.0
        //   idx 4: (13.5 * 2/3) + (12.0 * 1/3) = 9.0 + 4.0 = 13.0
        // DEMA = 2 * EMA1 - EMA2:
        //   idx 2: 2*11.5 - 11.0 = 23.0 - 11.0 = 12.0
        //   idx 3: 2*12.5 - 12.0 = 25.0 - 12.0 = 13.0
        //   idx 4: 2*13.5 - 13.0 = 27.0 - 13.0 = 14.0

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 5);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        if let Some(val) = out.get(2) {
            assert!((val - 12.0).abs() < 1e-4, "Expected ~12.0, got {}", val);
        } else {
            anyhow::bail!("Expected value at index 2");
        }

        if let Some(val) = out.get(3) {
            assert!((val - 13.0).abs() < 1e-4, "Expected ~13.0, got {}", val);
        } else {
            anyhow::bail!("Expected value at index 3");
        }

        if let Some(val) = out.get(4) {
            assert!((val - 14.0).abs() < 1e-4, "Expected ~14.0, got {}", val);
        } else {
            anyhow::bail!("Expected value at index 4");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        } else {
            anyhow::bail!("Expected error for empty data");
        }

        let df_normal = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_zero = calculate(&df_normal, 0);
        if let Err(e) = res_zero {
            assert_eq!(e.to_string(), "Period must be greater than 0");
        } else {
            anyhow::bail!("Expected error for zero period");
        }

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);

        let out_short = res_short.f64()?;
        assert!(out_short.get(0).is_none());
        assert!(out_short.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64 * 0.1)).collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 14)?;
        let s = result.f64()?;

        assert_eq!(s.len(), 100);
        assert!(s.get(99).is_some());

        if let Some(val) = s.get(99) {
            assert!(val > 90.0 && val < 120.0);
        }

        Ok(())
    }
}
