//! OBV - On-Balance Volume
//!
//! A momentum indicator that uses volume flow to predict changes in stock price.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate On-Balance Volume (OBV)
///
/// # Arguments
/// * `data` - DataFrame with "close" and "volume" columns
///
/// # Returns
/// Series with OBV values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::obv;
///
/// let df = df!(
///     "close" => &[10.0, 10.5, 10.2, 10.8],
///     "volume" => &[100.0, 200.0, 150.0, 300.0]
/// ).unwrap();
/// let result = obv::calculate(&df).unwrap();
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let volume = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?
        .f64()
        .context("Volume column must be numeric (f64)")?;

    let len = close.len();
    let mut obv_values: Vec<Option<f64>> = Vec::with_capacity(len);
    let mut current_obv = Decimal::ZERO;

    if len == 0 {
        return Ok(Series::new("obv", obv_values));
    }

    // First value is simply the volume (or 0 if volume is missing)
    let mut prev_close = match close.get(0) {
        Some(v) => Decimal::from_f64_retain(v).unwrap_or(Decimal::ZERO),
        None => Decimal::ZERO,
    };

    if let Some(vol) = volume.get(0) {
        if let Some(vol_dec) = Decimal::from_f64_retain(vol) {
            current_obv = vol_dec;
        }
    }
    obv_values.push(Some(current_obv.to_f64().unwrap_or(0.0)));

    for i in 1..len {
        let current_close_opt = close.get(i);
        let current_vol_opt = volume.get(i);

        match (current_close_opt, current_vol_opt) {
            (Some(c), Some(v)) => {
                if let (Some(c_dec), Some(v_dec)) = (Decimal::from_f64_retain(c), Decimal::from_f64_retain(v)) {
                    if c_dec > prev_close {
                        current_obv += v_dec;
                    } else if c_dec < prev_close {
                        current_obv -= v_dec;
                    }
                    // If c_dec == prev_close, current_obv remains the same

                    prev_close = c_dec;
                    obv_values.push(Some(current_obv.to_f64().unwrap_or(0.0)));
                } else {
                    obv_values.push(None);
                }
            }
            _ => {
                obv_values.push(None);
            }
        }
    }

    Ok(Series::new("obv", obv_values))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_values() -> Result<()> {
        // Reference values logic:
        // Day 1: Close 10, Vol 100 -> OBV 100
        // Day 2: Close 10.5 (> 10), Vol 200 -> OBV 100 + 200 = 300
        // Day 3: Close 10.2 (< 10.5), Vol 150 -> OBV 300 - 150 = 150
        // Day 4: Close 10.2 (== 10.2), Vol 300 -> OBV 150
        // Day 5: Close 10.8 (> 10.2), Vol 100 -> OBV 150 + 100 = 250

        let df = df!(
            "close" => &[10.0, 10.5, 10.2, 10.2, 10.8],
            "volume" => &[100.0, 200.0, 150.0, 300.0, 100.0]
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        assert_eq!(out.get(0), Some(100.0));
        assert_eq!(out.get(1), Some(300.0));
        assert_eq!(out.get(2), Some(150.0));
        assert_eq!(out.get(3), Some(150.0));
        assert_eq!(out.get(4), Some(250.0));

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Single data point
        let df_single = df!(
            "close" => &[10.0],
            "volume" => &[100.0]
        )?;
        let res_single = calculate(&df_single)?;
        let out_single = res_single.f64()?;
        assert_eq!(out_single.len(), 1);
        assert_eq!(out_single.get(0), Some(100.0));

        // Missing column
        let df_missing = df!(
            "close" => &[10.0]
        )?;
        let res_missing = calculate(&df_missing);
        assert!(res_missing.is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[50000.0, 51000.0, 49000.0, 49500.0],
            "volume" => &[10.5, 15.2, 20.1, 12.0]
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        assert_eq!(out.get(0), Some(10.5));

        // 51000 > 50000 => +15.2
        assert_eq!(out.get(1), Some(25.7));

        // 49000 < 51000 => -20.1
        // Floating point arithmetic issue: 25.7 - 20.1 = 5.599999999999998
        // We compare using precision up to 5 decimals or just hardcode the value returned by rust_decimal's `.to_f64()`
        let v2 = out.get(2).unwrap();
        assert!((v2 - 5.6).abs() < 1e-6);

        // 49500 > 49000 => +12.0
        let v3 = out.get(3).unwrap();
        assert!((v3 - 17.6).abs() < 1e-6);

        Ok(())
    }
}
