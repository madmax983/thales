//! Double Exponential Moving Average (DEMA)
//!
//! Calculates the Double Exponential Moving Average, which is designed to reduce the lag
//! inherent in traditional moving averages by subtracting a smoothed EMA from a doubled EMA.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use super::ema;

/// Calculate Double Exponential Moving Average (DEMA)
///
/// DEMA = (2 * EMA1) - EMA2
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
///
/// let df = df!(
///     "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
/// ).unwrap_or_default();
/// let result = dema::calculate(&df, 3).unwrap_or_default();
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

    for (i, val) in dema_values.iter_mut().enumerate().take(ema1_f64.len()) {
        if let (Some(e1), Some(e2)) = (ema1_f64.get(i), ema2_f64.get(i)) {
            let dec_e1 = Decimal::from_f64_retain(e1).context("Failed to convert EMA1 to Decimal")?;
            let dec_e2 = Decimal::from_f64_retain(e2).context("Failed to convert EMA2 to Decimal")?;

            let dema = (two * dec_e1) - dec_e2;
            *val = Some(dema.to_f64().context("Failed to convert Decimal to f64")?);
        }
    }

    Ok(Series::new("dema", dema_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_dema_known_values() -> Result<()> {
        let values: Vec<f64> = vec![
            10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0,
        ];
        let df = df!("close" => values)?;
        let result = calculate(&df, 3)?;
        let s = result.f64()?;

        assert_eq!(s.len(), 10);

        // DEMA calculation for period 3
        // Values will only be available after sufficient data points
        // Let's check the last value to ensure it matches expected calculations
        let last_val = s.get(9).unwrap_or(0.0);
        assert!(last_val > 0.0);

        // With a linear increase, DEMA will closely follow the price
        // For a simple linear trend of +1 per period, DEMA will be very close to the current price
        assert!((last_val - 19.0).abs() < 1.0);

        Ok(())
    }

    #[test]
    fn test_dema_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_normal = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());
        assert_eq!(res_zero.unwrap_err().to_string(), "Period must be greater than 0");

        let df_single = df!("close" => &[10.0])?;
        let res_single = calculate(&df_single, 5)?;
        assert_eq!(res_single.len(), 1);

        Ok(())
    }

    #[test]
    fn test_dema_realistic_data() -> Result<()> {
        let mut closes = Vec::new();
        for i in 0..100 {
            let val = 100.0 + (i as f64 * 0.1).sin() * 10.0;
            closes.push(val);
        }

        let df = df!("close" => closes)?;
        let result = calculate(&df, 14)?;

        assert_eq!(result.len(), 100);
        let out = result.f64()?;

        let valid_count = out.into_iter().filter(|x| x.is_some()).count();
        assert!(valid_count > 0);

        Ok(())
    }
}
