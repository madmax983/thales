//! Positive Volume Index (PVI)
//!
//! The Positive Volume Index (PVI) is a cumulative indicator that uses the change in volume
//! to decide when the not-so-smart money is active.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Positive Volume Index (PVI)
///
/// # Arguments
/// * `data` - DataFrame with "close" and "volume" columns
///
/// # Returns
/// Series with PVI values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let pvi = strategies::indicators::pvi::calculate(&df)?;
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .cast(&DataType::String)
        .context("Failed to cast close to String")?;
    let close_str = close.str().context("Failed to get close as str")?;

    let volume = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?
        .cast(&DataType::String)
        .context("Failed to cast volume to String")?;
    let volume_str = volume.str().context("Failed to get volume as str")?;

    let mut pvi_values: Vec<Option<String>> = Vec::with_capacity(close.len());

    if close.is_empty() {
        return Ok(Series::new("pvi", pvi_values));
    }

    let mut current_pvi = Decimal::new(1000, 0); // Base value 1000

    pvi_values.push(Some(current_pvi.to_string()));

    for i in 1..close.len() {
        let prev_close_opt = close_str.get(i - 1);
        let curr_close_opt = close_str.get(i);
        let prev_vol_opt = volume_str.get(i - 1);
        let curr_vol_opt = volume_str.get(i);

        match (prev_close_opt, curr_close_opt, prev_vol_opt, curr_vol_opt) {
            (Some(prev_close), Some(curr_close), Some(prev_vol), Some(curr_vol)) => {
                let prev_close_dec = Decimal::from_str(prev_close).ok();
                let curr_close_dec = Decimal::from_str(curr_close).ok();
                let prev_vol_dec = Decimal::from_str(prev_vol).ok();
                let curr_vol_dec = Decimal::from_str(curr_vol).ok();

                if let (Some(pc), Some(cc), Some(pv), Some(cv)) =
                    (prev_close_dec, curr_close_dec, prev_vol_dec, curr_vol_dec)
                {
                    if cv > pv && !pc.is_zero() {
                        let rate_of_change = (cc - pc) / pc;
                        current_pvi += current_pvi * rate_of_change;
                    }
                    pvi_values.push(Some(current_pvi.to_string()));
                } else {
                    pvi_values.push(None);
                }
            }
            _ => {
                pvi_values.push(None);
            }
        }
    }

    Ok(Series::new("pvi", pvi_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &["100", "105", "102", "104", "101"],
            "volume" => &["1000", "1200", "900", "1100", "800"]
        )
        .context("Failed to create DataFrame")?;

        let res = calculate(&df)?;
        let out = res.str().context("Failed to convert Series to str")?;

        if let Some(v0) = out.get(0) {
            let val = Decimal::from_str(v0).context("Failed to parse v0")?;
            assert!((val - Decimal::from_str("1000")?).abs() < Decimal::from_str("0.0001")?);
        } else {
            anyhow::bail!("Index 0 is None");
        }

        if let Some(v1) = out.get(1) {
            let val = Decimal::from_str(v1).context("Failed to parse v1")?;
            assert!((val - Decimal::from_str("1050")?).abs() < Decimal::from_str("0.0001")?);
        } else {
            anyhow::bail!("Index 1 is None");
        }

        if let Some(v2) = out.get(2) {
            let val = Decimal::from_str(v2).context("Failed to parse v2")?;
            assert!((val - Decimal::from_str("1050")?).abs() < Decimal::from_str("0.0001")?);
        } else {
            anyhow::bail!("Index 2 is None");
        }

        if let Some(v3) = out.get(3) {
            let val = Decimal::from_str(v3).context("Failed to parse v3")?;
            assert!((val - Decimal::from_str("1070.58823529")?).abs() < Decimal::from_str("0.0001")?);
        } else {
            anyhow::bail!("Index 3 is None");
        }

        if let Some(v4) = out.get(4) {
            let val = Decimal::from_str(v4).context("Failed to parse v4")?;
            assert!((val - Decimal::from_str("1070.58823529")?).abs() < Decimal::from_str("0.0001")?);
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

        let df_single = df!(
            "close" => &["100"],
            "volume" => &["1000"]
        )
        .context("Failed to create DataFrame")?;

        let res_single = calculate(&df_single)?;
        let out_single = res_single.str().context("Failed to convert")?;
        assert_eq!(out_single.len(), 1);
        if let Some(v) = out_single.get(0) {
            let val = Decimal::from_str(v).context("Failed to parse v")?;
            assert!((val - Decimal::from_str("1000")?).abs() < Decimal::from_str("0.0001")?);
        } else {
            anyhow::bail!("Index 0 is None");
        }

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &["10", "11", "12", "11", "10", "12"],
            "volume" => &["100", "150", "120", "130", "90", "110"]
        )
        .context("Failed to create DataFrame")?;

        let res = calculate(&df);
        assert!(res.is_ok());
        let s = res?;
        assert_eq!(s.len(), 6);
        Ok(())
    }
}
