//! Average Price - Calculates the average of Open, High, Low, and Close prices

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use std::str::FromStr;

/// Calculate Average Price (OHLC4)
///
/// # Arguments
/// * `data` - DataFrame with "open", "high", "low", "close" columns
///
/// # Returns
/// Series of type String with indicator values to guarantee zero floating-point imprecision.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...
/// // let result = average_price::calculate(&df)?;
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    let open_series = data.column("open").context("Missing open column")?.cast(&DataType::String)?;
    let high_series = data.column("high").context("Missing high column")?.cast(&DataType::String)?;
    let low_series = data.column("low").context("Missing low column")?.cast(&DataType::String)?;
    let close_series = data.column("close").context("Missing close column")?.cast(&DataType::String)?;

    let open_ca = open_series.str()?;
    let high_ca = high_series.str()?;
    let low_ca = low_series.str()?;
    let close_ca = close_series.str()?;

    let four = Decimal::from(4);

    let mut result_values = Vec::with_capacity(open_ca.len());

    for (((o_opt, h_opt), l_opt), c_opt) in open_ca
        .into_iter()
        .zip(high_ca)
        .zip(low_ca)
        .zip(close_ca)
    {
        let val = match (o_opt, h_opt, l_opt, c_opt) {
            (Some(o_str), Some(h_str), Some(l_str), Some(c_str)) => {
                let o = Decimal::from_str(o_str);
                let h = Decimal::from_str(h_str);
                let l = Decimal::from_str(l_str);
                let c = Decimal::from_str(c_str);
                if let (Ok(o), Ok(h), Ok(l), Ok(c)) = (o, h, l, c) {
                    let mut avg = (o + h + l + c) / four;
                    avg.normalize_assign();
                    Some(avg.to_string())
                } else {
                    None
                }
            }
            _ => None,
        };
        result_values.push(val);
    }

    Ok(Series::new("average_price", result_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "open" => &["10.0", "20.0"],
            "high" => &["15.0", "25.0"],
            "low" => &["5.0", "15.0"],
            "close" => &["10.0", "20.0"],
        )?;

        let result = calculate(&df)?;
        let out = result.str()?;

        // (10+15+5+10)/4 = 10
        assert_eq!(out.get(0), Some("10"));
        // (20+25+15+20)/4 = 20
        assert_eq!(out.get(1), Some("20"));

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Missing columns
        let df_missing = df!("open" => &["10.0"])?;
        let res_missing = calculate(&df_missing);
        assert!(res_missing.is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "open" => &["100.50", "102.00", "101.50"],
            "high" => &["105.00", "103.50", "104.00"],
            "low" => &["99.00", "100.50", "101.00"],
            "close" => &["102.50", "101.00", "103.50"],
        )?;

        let s = calculate(&df)?;
        let out = s.str()?;

        // 1: (100.50 + 105.00 + 99.00 + 102.50) / 4 = 407 / 4 = 101.75
        assert_eq!(out.get(0), Some("101.75"));
        // 2: (102.00 + 103.50 + 100.50 + 101.00) / 4 = 407 / 4 = 101.75
        assert_eq!(out.get(1), Some("101.75"));
        // 3: (101.50 + 104.00 + 101.00 + 103.50) / 4 = 410 / 4 = 102.50
        assert_eq!(out.get(2), Some("102.5"));

        Ok(())
    }
}
