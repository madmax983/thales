use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Balance of Power (BOP)
///
/// BOP = (Close - Open) / (High - Low)
///
/// # Arguments
/// * `data` - DataFrame with "open", "high", "low", "close" columns
///
/// # Returns
/// Series with BOP values
pub fn calculate(data: &DataFrame) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    let open = data
        .column("open")
        .context("DataFrame must contain 'open' column")?
        .f64()
        .context("Open column must be numeric (f64)")?;

    let high = data
        .column("high")
        .context("DataFrame must contain 'high' column")?
        .f64()
        .context("High column must be numeric (f64)")?;

    let low = data
        .column("low")
        .context("DataFrame must contain 'low' column")?
        .f64()
        .context("Low column must be numeric (f64)")?;

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let mut bop_values: Vec<Option<f64>> = Vec::with_capacity(open.len());

    for i in 0..open.len() {
        if let (Some(o_val), Some(h_val), Some(l_val), Some(c_val)) =
            (open.get(i), high.get(i), low.get(i), close.get(i))
        {
            if let (Some(o), Some(h), Some(l), Some(c)) = (
                Decimal::from_f64_retain(o_val),
                Decimal::from_f64_retain(h_val),
                Decimal::from_f64_retain(l_val),
                Decimal::from_f64_retain(c_val),
            ) {
                let range = h - l;
                if range.is_zero() {
                    // Avoid division by zero
                    bop_values.push(Some(0.0));
                } else {
                    let bop = (c - o) / range;
                    bop_values.push(bop.to_f64());
                }
            } else {
                bop_values.push(None);
            }
        } else {
            bop_values.push(None);
        }
    }

    Ok(Series::new("bop", bop_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_bop_known_values() -> Result<()> {
        let df = df!(
            "open" => &[10.0, 11.0, 10.0, 15.0],
            "high" => &[15.0, 12.0, 15.0, 15.0],
            "low" => &[5.0, 9.0, 10.0, 15.0],
            "close" => &[15.0, 10.5, 12.0, 15.0],
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        // idx 0: open=10, high=15, low=5, close=15
        // BOP = (15 - 10) / (15 - 5) = 5 / 10 = 0.5
        assert_eq!(out.get(0), Some(0.5));

        // idx 1: open=11, high=12, low=9, close=10.5
        // BOP = (10.5 - 11) / (12 - 9) = -0.5 / 3 = -0.166666...
        let val = out.get(1).unwrap_or(0.0);
        assert!((val - -0.16666666666666666).abs() < 1e-6);

        // idx 2: open=10, high=15, low=10, close=12
        // BOP = (12 - 10) / (15 - 10) = 2 / 5 = 0.4
        assert_eq!(out.get(2), Some(0.4));

        // idx 3: open=15, high=15, low=15, close=15
        // BOP = (15 - 15) / (15 - 15) = 0 / 0 -> 0.0 (or None if division by zero)
        assert_eq!(out.get(3), Some(0.0));

        Ok(())
    }

    #[test]
    fn test_bop_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        Ok(())
    }

    #[test]
    fn test_bop_realistic_data() -> Result<()> {
        let df = df!(
            "open" => &[150.0, 152.0, 151.0],
            "high" => &[155.0, 154.0, 153.0],
            "low" => &[149.0, 150.0, 148.0],
            "close" => &[152.0, 151.0, 150.0],
        )?;

        let res = calculate(&df)?;
        assert_eq!(res.len(), 3);
        Ok(())
    }
}
