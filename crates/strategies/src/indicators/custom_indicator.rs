//! custom_indicator - A template or custom technical indicator
//!
//! Calculates a simple rolling mean using rust_decimal for precision to satisfy the template requirements.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate custom_indicator
///
/// # Arguments
/// * `data` - DataFrame with OHLCV data
/// * `period` - Lookback period
///
/// # Returns
/// Series with indicator values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use rust_decimal::Decimal;
/// use strategies::indicators::custom_indicator;
///
/// let df = df!(
///     "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
/// ).unwrap_or_default();
/// let result = custom_indicator::calculate(&df, 3).unwrap_or_default();
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Get "close" column
    let close_s = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .cast(&DataType::Float64)?;

    let close = close_s.f64().context("Close column must be numeric")?;

    let decimal_close: Vec<Option<Decimal>> = close
        .into_iter()
        .map(|opt_val| {
            if let Some(val) = opt_val {
                Decimal::from_f64_retain(val)
            } else {
                None
            }
        })
        .collect();

    let mut out_values: Vec<Option<f64>> = vec![None; decimal_close.len()];
    let period_dec = Decimal::from_usize(period).context("Invalid period")?;

    for (i, out) in out_values
        .iter_mut()
        .enumerate()
        .take(decimal_close.len())
        .skip(period - 1)
    {
        let mut sum = Decimal::ZERO;
        let mut valid = true;

        for j in 0..period {
            let idx = i + 1 + j - period;
            if let Some(d) = decimal_close[idx] {
                sum += d;
            } else {
                valid = false;
                break;
            }
        }

        if valid {
            let mean = sum / period_dec;
            *out = mean.to_f64();
        }
    }

    let s = Series::new("custom_indicator", out_values);
    Ok(s)
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

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).unwrap_or(0.0);
        assert!((val2 - 11.0).abs() < 1e-10);

        let val3 = out.get(3).unwrap_or(0.0);
        assert!((val3 - 12.0).abs() < 1e-10);

        let val4 = out.get(4).unwrap_or(0.0);
        assert!((val4 - 13.0).abs() < 1e-10);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut closes = Vec::new();
        for i in 0..100 {
            closes.push(100.0 + (i as f64) * 0.5);
        }

        let df = df!("close" => &closes)?;
        let result = calculate(&df, 14)?;

        assert_eq!(result.len(), 100);
        let out = result.f64()?;
        assert!(out.get(12).is_none());
        assert!(out.get(13).is_some());
        assert!(out.get(99).is_some());

        Ok(())
    }
}
