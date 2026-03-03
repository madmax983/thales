//! ROC - Rate of Change
//!
//! A momentum oscillator that measures the percentage change in price between the current price and the price a certain number of periods ago.
//! ROC is used to identify overbought and oversold conditions, as well as trend reversals and divergences.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Rate of Change (ROC)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period (e.g., 9 or 14)
///
/// # Returns
/// Series with ROC values. The first `period` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let roc = strategies::indicators::roc::calculate(&df, 9)?;
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

    let mut roc_values: Vec<Option<f64>> = vec![None; close.len()];

    if close.len() <= period {
        return Ok(Series::new("roc", roc_values));
    }

    let hundred = Decimal::new(100, 0);

    for (i, roc_val) in roc_values
        .iter_mut()
        .enumerate()
        .take(close.len())
        .skip(period)
    {
        let curr_opt = close.get(i);
        let prev_opt = close.get(i - period);

        if let (Some(curr), Some(prev)) = (curr_opt, prev_opt) {
            let curr_dec = Decimal::from_f64_retain(curr).unwrap_or(Decimal::ZERO);
            let prev_dec = Decimal::from_f64_retain(prev).unwrap_or(Decimal::ZERO);

            if prev_dec.is_zero() {
                // If previous price is zero, we can't calculate ROC. Use None.
                *roc_val = None;
            } else {
                let change = curr_dec - prev_dec;
                let roc = (change / prev_dec) * hundred;
                *roc_val = roc.to_f64();
            }
        } else {
            *roc_val = None;
        }
    }

    Ok(Series::new("roc", roc_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        // Period 2
        // Data: [10.0, 11.0, 12.0, 9.0]
        // i=0: None
        // i=1: None
        // i=2: ROC = ((12.0 - 10.0) / 10.0) * 100 = 20.0
        // i=3: ROC = ((9.0 - 11.0) / 11.0) * 100 = -18.1818...

        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 9.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 4);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).unwrap();
        assert!((val2 - 20.0).abs() < 1e-4, "Expected ~20.0, got {}", val2);

        let val3 = out.get(3).unwrap();
        assert!(
            (val3 - -18.181818).abs() < 1e-4,
            "Expected ~-18.1818, got {}",
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
        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.f64()?;
        assert_eq!(out.len(), 2);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        // Zero period
        let df_normal = df!("close" => &[10.0, 11.0])?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());

        // Zero previous price
        let df_zero_price = df!("close" => &[0.0, 10.0])?;
        let res_zero_price = calculate(&df_zero_price, 1)?;
        let out_zero_price = res_zero_price.f64()?;
        assert!(out_zero_price.get(1).is_none()); // Division by zero should yield None or be handled safely

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
        assert!(s.f64()?.get(13).is_none());
        assert!(s.f64()?.get(14).is_some());

        Ok(())
    }
}
