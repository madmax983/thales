//! Negative Volume Index (NVI)
//!
//! The Negative Volume Index (NVI) is a cumulative indicator that uses the change in volume
//! to decide when the smart money is active.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Negative Volume Index (NVI)
///
/// # Arguments
/// * `data` - DataFrame with "close" and "volume" columns
///
/// # Returns
/// Series with NVI values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let nvi = strategies::indicators::nvi::calculate(&df)?;
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

    let mut nvi_values: Vec<Option<f64>> = Vec::with_capacity(close.len());

    if close.len() == 0 {
        return Ok(Series::new("nvi", nvi_values));
    }

    let mut current_nvi = Decimal::new(1000, 0); // Base value 1000

    nvi_values.push(Some(current_nvi.to_f64().context("Failed to convert Decimal to f64")?));

    for i in 1..close.len() {
        let prev_close_opt = close.get(i - 1);
        let curr_close_opt = close.get(i);
        let prev_vol_opt = volume.get(i - 1);
        let curr_vol_opt = volume.get(i);

        match (prev_close_opt, curr_close_opt, prev_vol_opt, curr_vol_opt) {
            (Some(prev_close), Some(curr_close), Some(prev_vol), Some(curr_vol)) => {
                let prev_close_dec = Decimal::from_f64_retain(prev_close);
                let curr_close_dec = Decimal::from_f64_retain(curr_close);
                let prev_vol_dec = Decimal::from_f64_retain(prev_vol);
                let curr_vol_dec = Decimal::from_f64_retain(curr_vol);

                if let (Some(pc), Some(cc), Some(pv), Some(cv)) =
                    (prev_close_dec, curr_close_dec, prev_vol_dec, curr_vol_dec)
                {
                    if cv < pv {
                        if !pc.is_zero() {
                            let rate_of_change = (cc - pc) / pc;
                            current_nvi += current_nvi * rate_of_change;
                        }
                    }
                    nvi_values.push(Some(current_nvi.to_f64().context("Failed to convert Decimal to f64")?));
                } else {
                    nvi_values.push(None);
                }
            }
            _ => {
                nvi_values.push(None);
            }
        }
    }

    Ok(Series::new("nvi", nvi_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let mut df = df!(
            "close" => &[100, 105, 102, 104, 101],
            "volume" => &[1000, 1200, 900, 1100, 800]
        ).context("Failed to create DataFrame")?;
        df.try_apply("close", |s| s.cast(&DataType::Float64)).context("Failed to cast close")?;
        df.try_apply("volume", |s| s.cast(&DataType::Float64)).context("Failed to cast volume")?;

        let res = calculate(&df)?;
        let out: &Float64Chunked = res.f64().context("Failed to convert Series to f64")?;

        if let Some(v0) = out.get(0) {
            assert!((v0 - 1000.0).abs() < 1e-4);
        } else {
            anyhow::bail!("Index 0 is None");
        }

        if let Some(v1) = out.get(1) {
            assert!((v1 - 1000.0).abs() < 1e-4);
        } else {
            anyhow::bail!("Index 1 is None");
        }

        if let Some(v2) = out.get(2) {
            assert!((v2 - 971.42857).abs() < 1e-4);
        } else {
            anyhow::bail!("Index 2 is None");
        }

        if let Some(v3) = out.get(3) {
            assert!((v3 - 971.42857).abs() < 1e-4);
        } else {
            anyhow::bail!("Index 3 is None");
        }

        if let Some(v4) = out.get(4) {
            assert!((v4 - 943.40659).abs() < 1e-4);
        } else {
            anyhow::bail!("Index 4 is None");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        let mut df_single = df!(
            "close" => &[100],
            "volume" => &[1000]
        ).context("Failed to create DataFrame")?;
        df_single.try_apply("close", |s| s.cast(&DataType::Float64)).context("Failed to cast")?;
        df_single.try_apply("volume", |s| s.cast(&DataType::Float64)).context("Failed to cast")?;

        let res_single = calculate(&df_single)?;
        let out_single: &Float64Chunked = res_single.f64().context("Failed to convert")?;
        assert_eq!(out_single.len(), 1);
        if let Some(v) = out_single.get(0) {
            assert!((v - 1000.0).abs() < 1e-4);
        } else {
            anyhow::bail!("Index 0 is None");
        }

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut df = df!(
            "close" => &[10, 11, 12, 11, 10, 12],
            "volume" => &[100, 150, 120, 130, 90, 110]
        ).context("Failed to create DataFrame")?;
        df.try_apply("close", |s| s.cast(&DataType::Float64)).context("Failed to cast")?;
        df.try_apply("volume", |s| s.cast(&DataType::Float64)).context("Failed to cast")?;

        let res = calculate(&df);
        assert!(res.is_ok());
        let s = res?;
        assert_eq!(s.len(), 6);
        Ok(())
    }
}
