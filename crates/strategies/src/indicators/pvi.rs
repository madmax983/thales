//! Positive Volume Index (PVI)
//!
//! The Positive Volume Index (PVI) is a cumulative indicator that uses the change in volume
//! to decide when the less smart money is active.

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
/// Series of strings representing PVI values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let pvi = strategies::indicators::pvi::calculate(&df)?;
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    let close_series = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .cast(&DataType::String)
        .context("Failed to cast close to String")?;

    let volume_series = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?
        .cast(&DataType::String)
        .context("Failed to cast volume to String")?;

    let close_str = close_series.str().context("Close column must be String")?;
    let volume_str = volume_series
        .str()
        .context("Volume column must be String")?;

    let mut pvi_values: Vec<Option<String>> = Vec::with_capacity(close_str.len());

    if close_str.is_empty() {
        return Ok(Series::new("pvi", pvi_values));
    }

    let mut current_pvi = Decimal::from_str("1000").context("Failed to parse base PVI")?;

    pvi_values.push(Some(current_pvi.to_string()));

    for i in 1..close_str.len() {
        let prev_close_opt = close_str.get(i - 1);
        let curr_close_opt = close_str.get(i);
        let prev_vol_opt = volume_str.get(i - 1);
        let curr_vol_opt = volume_str.get(i);

        match (prev_close_opt, curr_close_opt, prev_vol_opt, curr_vol_opt) {
            (Some(prev_close_s), Some(curr_close_s), Some(prev_vol_s), Some(curr_vol_s)) => {
                let pc = Decimal::from_str(prev_close_s)
                    .map_err(|e| anyhow::anyhow!("Failed to parse prev close: {}", e))?;
                let cc = Decimal::from_str(curr_close_s)
                    .map_err(|e| anyhow::anyhow!("Failed to parse curr close: {}", e))?;
                let pv = Decimal::from_str(prev_vol_s)
                    .map_err(|e| anyhow::anyhow!("Failed to parse prev volume: {}", e))?;
                let cv = Decimal::from_str(curr_vol_s)
                    .map_err(|e| anyhow::anyhow!("Failed to parse curr volume: {}", e))?;

                if cv > pv && !pc.is_zero() {
                    let rate_of_change = (cc - pc) / pc;
                    current_pvi += current_pvi * rate_of_change;
                }
                pvi_values.push(Some(current_pvi.to_string()));
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

        let v0 = Decimal::from_str(out.get(0).context("Index 0 is None")?)?;
        assert!((v0 - Decimal::from_str("1000")?).abs() < Decimal::from_str("0.0001")?);

        let v1 = Decimal::from_str(out.get(1).context("Index 1 is None")?)?;
        assert!((v1 - Decimal::from_str("1050")?).abs() < Decimal::from_str("0.0001")?);

        let v2 = Decimal::from_str(out.get(2).context("Index 2 is None")?)?;
        assert!((v2 - Decimal::from_str("1050")?).abs() < Decimal::from_str("0.0001")?);

        let v3 = Decimal::from_str(out.get(3).context("Index 3 is None")?)?;
        let expected_v3 = Decimal::from_str("1070.5882352941176470588235294")?;
        assert!((v3 - expected_v3).abs() < Decimal::from_str("0.001")?);

        let v4 = Decimal::from_str(out.get(4).context("Index 4 is None")?)?;
        assert!((v4 - expected_v3).abs() < Decimal::from_str("0.001")?);

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
        let v = Decimal::from_str(out_single.get(0).context("Index 0 is None")?)?;
        assert!((v - Decimal::from_str("1000")?).abs() < Decimal::from_str("0.0001")?);

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
