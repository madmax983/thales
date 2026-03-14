//! Volume Price Trend (VPT)
//!
//! Calculates the Volume Price Trend (VPT), a momentum indicator that uses volume
//! to confirm price trends or warn of potential reversals.
//!
//! VPT = Previous VPT + Volume * ((Current Close - Previous Close) / Previous Close)

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Volume Price Trend (VPT)
///
/// # Arguments
/// * `data` - DataFrame with "close" and "volume" columns
///
/// # Returns
/// Series with VPT values. The first value will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let vpt = strategies::indicators::vpt::calculate(&df)?;
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    // Validate inputs
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

    let mut vpt_values: Vec<Option<f64>> = vec![None; close.len()];
    let mut current_vpt = Decimal::ZERO;
    let mut initialized = false;

    for i in 1..close.len() {
        let curr_close_opt = close.get(i);
        let prev_close_opt = close.get(i - 1);
        let vol_opt = volume.get(i);

        if let (Some(curr_c), Some(prev_c), Some(v)) = (curr_close_opt, prev_close_opt, vol_opt) {
            let curr_c_dec = Decimal::from_f64_retain(curr_c).unwrap_or(Decimal::ZERO);
            let prev_c_dec = Decimal::from_f64_retain(prev_c).unwrap_or(Decimal::ZERO);
            let v_dec = Decimal::from_f64_retain(v).unwrap_or(Decimal::ZERO);

            if prev_c_dec.is_zero() {
                // Cannot divide by zero
                vpt_values[i] = None;
                continue;
            }

            let price_change_pct = (curr_c_dec - prev_c_dec) / prev_c_dec;
            let vpt_change = v_dec * price_change_pct;

            current_vpt += vpt_change;
            vpt_values[i] = Some(current_vpt.to_f64().unwrap_or(0.0));
            initialized = true;
        } else if initialized {
            // Carry forward previous value if data is missing, similar to OBV
            vpt_values[i] = Some(current_vpt.to_f64().unwrap_or(0.0));
        } else {
            vpt_values[i] = None;
        }
    }

    Ok(Series::new("vpt", vpt_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        // Data points:
        // i=0: Close=10.0, Vol=100.0 (VPT=null)
        // i=1: Close=11.0, Vol=50.0  -> Price change = (11-10)/10 = 0.1 -> VPT change = 50 * 0.1 = 5.0 -> VPT = 5.0
        // i=2: Close=10.5, Vol=20.0  -> Price change = (10.5-11)/11 = -0.04545... -> VPT change = 20 * -0.04545... = -0.90909... -> VPT = 4.09090...
        // i=3: Close=10.5, Vol=10.0  -> Price change = (10.5-10.5)/10.5 = 0.0 -> VPT change = 0 -> VPT = 4.09090...
        // i=4: Close=12.0, Vol=100.0 -> Price change = (12-10.5)/10.5 = 0.142857... -> VPT change = 100 * 0.142857... = 14.285714... -> VPT = 18.376623...

        let df = df!(
            "close" => &[10.0, 11.0, 10.5, 10.5, 12.0],
            "volume" => &[100.0, 50.0, 20.0, 10.0, 100.0]
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 5);
        assert!(out.get(0).is_none());

        let val1 = out.get(1).unwrap();
        assert!((val1 - 5.0).abs() < 1e-4, "Expected ~5.0, got {}", val1);

        let val2 = out.get(2).unwrap();
        assert!(
            (val2 - 4.090909).abs() < 1e-4,
            "Expected ~4.0909, got {}",
            val2
        );

        let val3 = out.get(3).unwrap();
        assert!(
            (val3 - 4.090909).abs() < 1e-4,
            "Expected ~4.0909, got {}",
            val3
        );

        let val4 = out.get(4).unwrap();
        assert!(
            (val4 - 18.376623).abs() < 1e-4,
            "Expected ~18.3766, got {}",
            val4
        );

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Missing column
        let df_missing = df!("close" => &[10.0])?;
        let res_missing = calculate(&df_missing);
        assert!(res_missing.is_err());

        // Zero previous price
        let df_zero_price = df!("close" => &[0.0, 10.0], "volume" => &[100.0, 100.0])?;
        let res_zero_price = calculate(&df_zero_price)?;
        let out_zero_price = res_zero_price.f64()?;
        assert!(out_zero_price.get(1).is_none()); // Division by zero

        // Missing data
        let s_close = Series::new("close", &[Some(10.0), None, Some(12.0)]);
        let s_vol = Series::new("volume", &[100.0, 100.0, 100.0]);
        let df_nulls = DataFrame::new(vec![s_close, s_vol])?;
        let res_nulls = calculate(&df_nulls)?;
        let out_nulls = res_nulls.f64()?;

        assert!(out_nulls.get(0).is_none());
        assert!(out_nulls.get(1).is_none());
        assert!(out_nulls.get(2).is_none()); // Since i-1 (which is index 1) is None, it cannot calculate change

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let volumes: Vec<f64> = (0..100)
            .map(|i| 1000.0 + (i as f64 * 0.2).cos() * 500.0)
            .collect();

        let df = df!(
            "close" => values,
            "volume" => volumes
        )?;

        let result = calculate(&df);
        assert!(result.is_ok());
        let s = result?;
        assert_eq!(s.len(), 100);
        assert!(s.f64()?.get(0).is_none());
        assert!(s.f64()?.get(1).is_some());
        assert!(s.f64()?.get(99).is_some());

        Ok(())
    }
}
