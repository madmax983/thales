//! WMA - Weighted Moving Average
//!
//! Calculates the Weighted Moving Average (WMA) of price data over a specified lookback period.
//! WMA places a greater weight and significance on the most recent data points,
//! but uses a linearly decreasing weight instead of an exponential one like EMA.

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
/// use strategies::indicators::wma;
/// use polars::prelude::*;
///
/// // let df = ...;
/// // let wma_series = wma::calculate(&df, 14)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
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

    let close_vec: Vec<Option<f64>> = close.into_iter().collect();
    let mut wma_values: Vec<Option<f64>> = Vec::with_capacity(close.len());

    let period_dec = Decimal::from_usize(period).context("Invalid period")?;
    // Sum of weights: n * (n + 1) / 2
    let sum_weights = (period_dec * (period_dec + Decimal::ONE)) / Decimal::new(2, 0);

    for i in 0..close_vec.len() {
        if i < period - 1 {
            wma_values.push(None);
            continue;
        }

        let mut current_wma = Decimal::ZERO;
        let mut valid_window = true;

        for j in 0..period {
            let idx = i.saturating_sub(period - 1).saturating_add(j);
            if let Some(val) = close_vec.get(idx).copied().flatten() {
                if let Some(val_dec) = Decimal::from_f64_retain(val) {
                    let weight = Decimal::from_usize(j + 1).unwrap_or(Decimal::ZERO);
                    current_wma += val_dec * weight;
                } else {
                    valid_window = false;
                    break;
                }
            } else {
                valid_window = false;
                break;
            }
        }

        if valid_window {
            current_wma /= sum_weights;
            wma_values.push(current_wma.to_f64());
        } else {
            wma_values.push(None);
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
        // Data: [10.0, 15.0, 20.0, 25.0]
        // Period: 3
        // Weights: 1, 2, 3. Sum = 6.
        // i=0: None
        // i=1: None
        // i=2: window [10, 15, 20]. WMA = (10*1 + 15*2 + 20*3) / 6 = (10 + 30 + 60) / 6 = 100 / 6 = 16.666...
        // i=3: window [15, 20, 25]. WMA = (15*1 + 20*2 + 25*3) / 6 = (15 + 40 + 75) / 6 = 130 / 6 = 21.666...

        let df = df!(
            "close" => &[10.0, 15.0, 20.0, 25.0]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 4);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        if let Some(val) = out.get(2) {
            assert!((val - 16.666666).abs() < 1e-4);
        } else {
            panic!("Expected value at index 2");
        }

        if let Some(val) = out.get(3) {
            assert!((val - 21.666666).abs() < 1e-4);
        } else {
            panic!("Expected value at index 3");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        } else {
            panic!("Expected error for empty data");
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
        if let Err(e) = res_zero {
            assert_eq!(e.to_string(), "Period must be greater than 0");
        } else {
            panic!("Expected error for zero period");
        }

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64 * 0.5)).collect();
        let df = df!("close" => values)?;

        let result = calculate(&df, 14)?;
        let s = result.f64()?;

        assert_eq!(s.len(), 100);
        assert!(s.get(12).is_none());
        assert!(s.get(13).is_some());

        // Since it's a linear upward trend, WMA should be higher than SMA,
        // but we just check it produces reasonable positive numbers.
        if let Some(last_val) = s.get(99) {
            assert!(last_val > 100.0);
        } else {
            panic!("Expected value at index 99");
        }

        Ok(())
    }
}
