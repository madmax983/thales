//! VROC - Volume Rate of Change
//!
//! Measures the percentage change in volume between the current volume and the volume a certain number of periods ago.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::str::FromStr;

/// Calculate Volume Rate of Change (VROC)
///
/// # Arguments
/// * `data` - DataFrame with "volume" column
/// * `period` - Lookback period (e.g., 9 or 14)
///
/// # Returns
/// Series with VROC values. The first `period` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let vroc = strategies::indicators::vroc::calculate(&df, 9)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.is_empty() {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let volume = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?
        .cast(&DataType::String)?;
    let volume_ca = volume.str()?;

    let len = data.height();
    let mut vroc_values: Vec<Option<String>> = vec![None; len];

    if len <= period {
        return Ok(Series::new("vroc", vroc_values));
    }

    let hundred = Decimal::from(100);

    let volume_vec: Vec<Option<Decimal>> = volume_ca
        .into_iter()
        .map(|opt_str| {
            if let Some(s) = opt_str {
                if s == "NaN" || s.is_empty() {
                    Ok(None)
                } else {
                    let dec =
                        Decimal::from_str(s).context(format!("Failed to parse volume: {}", s))?;
                    Ok(Some(dec))
                }
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>>>()?;

    for i in period..len {
        let curr_opt = volume_vec[i];
        let prev_opt = volume_vec[i - period];

        if let (Some(curr), Some(prev)) = (curr_opt, prev_opt) {
            if prev.is_zero() {
                // Cannot divide by zero
                vroc_values[i] = None;
            } else {
                let change = curr - prev;
                let vroc = (change / prev) * hundred;
                vroc_values[i] = Some(vroc.to_string());
            }
        } else {
            vroc_values[i] = None;
        }
    }

    Ok(Series::new("vroc", vroc_values))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "volume" => &["100.0", "110.0", "120.0", "90.0"]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.str()?;

        assert_eq!(out.len(), 4);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        assert_eq!(out.get(2).unwrap_or("0"), "20.00");

        let expected = ((Decimal::from_str("90.0")? - Decimal::from_str("110.0")?)
            / Decimal::from_str("110.0")?)
            * Decimal::from_str("100")?;
        assert_eq!(out.get(3).unwrap_or("0"), expected.to_string());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());

        let df_short = df!("volume" => &["100.0", "110.0"])?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.str()?;
        assert_eq!(out.len(), 2);
        assert!(out.get(0).is_none());

        let df_zero_period = df!("volume" => &["100.0", "110.0"])?;
        assert!(calculate(&df_zero_period, 0).is_err());

        let df_zero_prev = df!("volume" => &["0.0", "100.0"])?;
        let res_zero = calculate(&df_zero_prev, 1)?;
        assert!(res_zero.str()?.get(1).is_none()); // division by zero returns None

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "volume" => &["100.5", "101.2", "105.8", "99.1", "102.4"]
        )?;
        let result = calculate(&df, 2)?;
        assert_eq!(result.len(), 5);
        Ok(())
    }
}
