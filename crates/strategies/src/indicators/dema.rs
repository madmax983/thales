//! Double Exponential Moving Average (DEMA)
//!
//! DEMA = (2 * EMA) - EMA(EMA)
//! It reduces the lag of traditional exponential moving averages.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;

use super::ema;

/// Calculate Double Exponential Moving Average (DEMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with indicator values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::dema;
///
/// let df = df!("close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0]).unwrap();
/// let result = dema::calculate(&df, 3).unwrap();
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

    let ema1_f64 = ema1_series.cast(&DataType::Float64)?;
    let ema1_f64_chunked = ema1_f64.f64().context("EMA1 must be numeric")?;

    let ema2_f64 = ema2_series.cast(&DataType::Float64)?;
    let ema2_f64_chunked = ema2_f64.f64().context("EMA2 must be numeric")?;

    let mut dema_values: Vec<Option<f64>> = vec![None; ema1_f64_chunked.len()];
    let two = Decimal::new(2, 0);

    for (val, (e1_opt, e2_opt)) in dema_values
        .iter_mut()
        .zip(ema1_f64_chunked.into_iter().zip(ema2_f64_chunked.into_iter()))
    {
        if let (Some(e1), Some(e2)) = (e1_opt, e2_opt) {
            if let (Some(e1_dec), Some(e2_dec)) =
                (Decimal::from_f64_retain(e1), Decimal::from_f64_retain(e2))
            {
                let dema = (two * e1_dec) - e2_dec;
                if let Some(dema_f64) = dema.to_f64() {
                    *val = Some(dema_f64);
                }
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

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 5);

        // Let's verify our expectations for Period=3
        // index 0: 10.0 (ema1: None)
        // index 1: 11.0 (ema1: None)
        // index 2: 12.0 (ema1: SMA(10,11,12) = 11.0) -> ema2: None
        // index 3: 13.0 (ema1: (13 * 0.5) + (11 * 0.5) = 12.0) -> ema2: None
        // index 4: 14.0 (ema1: (14 * 0.5) + (12 * 0.5) = 13.0) -> ema2: SMA(11,12,13) = 12.0

        // Therefore, at index 4:
        // ema1 = 13.0
        // ema2 = 12.0
        // DEMA = 2*ema1 - ema2 = 2*13.0 - 12.0 = 26.0 - 12.0 = 14.0

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());
        assert!(out.get(3).is_none());
        assert_eq!(out.get(4), Some(14.0));

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..50).map(|i| 100.0 + (i as f64 * 0.5)).collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 5)?;
        let s = result.f64()?;

        assert_eq!(s.len(), 50);
        assert!(s.get(49).is_some());

        Ok(())
    }
}