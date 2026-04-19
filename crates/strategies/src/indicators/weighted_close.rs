//! Weighted Close - The weighted average of high, low, and close prices ((High + Low + Close * 2) / 4).

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::Decimal;
use std::str::FromStr;

/// Calculate Weighted Close
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns
///
/// # Returns
/// Series with Weighted Close values (as String)
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...
/// // let result = calculate(&df)?;
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    if data.is_empty() {
        anyhow::bail!("Data cannot be empty");
    }

    let high = data
        .column("high")
        .context("DataFrame must contain 'high' column")?
        .cast(&DataType::String)?;
    let high_ca = high.str()?;

    let low = data
        .column("low")
        .context("DataFrame must contain 'low' column")?
        .cast(&DataType::String)?;
    let low_ca = low.str()?;

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .cast(&DataType::String)?;
    let close_ca = close.str()?;

    let len = data.height();
    let mut weighted_closes = Vec::with_capacity(len);

    let iter = high_ca.into_iter().zip(low_ca).zip(close_ca);

    for (i, ((h_opt, l_opt), c_opt)) in iter.enumerate() {
        if let (Some(h_str), Some(l_str), Some(c_str)) = (h_opt, l_opt, c_opt) {
            if h_str == "NaN" || l_str == "NaN" || c_str == "NaN" {
                anyhow::bail!("NaN value found at index {}", i);
            }

            let h_dec =
                Decimal::from_str(h_str).context("Failed to parse high price as Decimal")?;
            let l_dec = Decimal::from_str(l_str).context("Failed to parse low price as Decimal")?;
            let c_dec =
                Decimal::from_str(c_str).context("Failed to parse close price as Decimal")?;

            let wc = (h_dec + l_dec + (c_dec * Decimal::from(2))) / Decimal::from(4);
            weighted_closes.push(wc.to_string());
        } else {
            anyhow::bail!("Missing data or null value at index {}", i);
        }
    }

    Ok(Series::new("weighted_close", weighted_closes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" => &["10.0", "12.0", "15.0"],
            "low" => &["5.0", "8.0", "10.0"],
            "close" => &["7.0", "10.0", "14.0"]
        )?;
        let res = calculate(&df)?;
        let s = res.str()?;

        let wc1 = (Decimal::from_str("10.0")?
            + Decimal::from_str("5.0")?
            + Decimal::from_str("7.0")? * Decimal::from(2))
            / Decimal::from(4);
        let wc2 = (Decimal::from_str("12.0")?
            + Decimal::from_str("8.0")?
            + Decimal::from_str("10.0")? * Decimal::from(2))
            / Decimal::from(4);
        let wc3 = (Decimal::from_str("15.0")?
            + Decimal::from_str("10.0")?
            + Decimal::from_str("14.0")? * Decimal::from(2))
            / Decimal::from(4);

        assert_eq!(s.get(0).unwrap_or(""), wc1.to_string());
        assert_eq!(s.get(1).unwrap_or(""), wc2.to_string());
        assert_eq!(s.get(2).unwrap_or(""), wc3.to_string());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df = DataFrame::default();
        assert!(calculate(&df).is_err());

        let df_single = df!(
            "high" => &["10.0"],
            "low" => &["10.0"],
            "close" => &["10.0"]
        )?;
        let res = calculate(&df_single)?;
        let expected = Decimal::from_str("10.0")?;
        assert_eq!(res.str()?.get(0).unwrap_or(""), expected.to_string());

        let df_nan = df!(
            "high" => &["NaN"],
            "low" => &["10.0"],
            "close" => &["10.0"]
        )?;
        assert!(calculate(&df_nan).is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "high" => &["10.5", "11.2", "11.8"],
            "low" => &["10.1", "10.8", "11.0"],
            "close" => &["10.4", "11.1", "11.5"]
        )?;
        let res = calculate(&df)?;
        assert_eq!(res.len(), 3);
        Ok(())
    }
}
