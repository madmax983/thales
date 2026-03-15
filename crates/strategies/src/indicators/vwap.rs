//! VWAP - Volume Weighted Average Price

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate VWAP
///
/// VWAP is an intraday calculation. This implementation assumes the input data
/// represents a single trading session or that VWAP should be calculated cumulatively
/// across the entire provided DataFrame. If session-based reset is required,
/// the data should be grouped and calculated per session.
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close", "volume" and "timestamp" columns
///
/// # Returns
/// Series with indicator values as `f64` (Polars Series format)
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ... load data
/// // let result = strategies::indicators::vwap::calculate(&df)?;
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    // Extract columns. We expect f64 for price/volume in the DataFrame as per polars norm,
    // but we immediately convert to Decimal for calculations.
    let high = data.column("high").context("Missing 'high' column")?.f64()?;
    let low = data.column("low").context("Missing 'low' column")?.f64()?;
    let close = data.column("close").context("Missing 'close' column")?.f64()?;
    let volume = data.column("volume").context("Missing 'volume' column")?.f64()?;

    // Validate timestamp exists
    let _timestamp = data.column("timestamp").context("Missing 'timestamp' column")?;

    let mut vwap_values: Vec<Option<f64>> = Vec::with_capacity(data.height());

    let mut cum_vol_price = Decimal::ZERO;
    let mut cum_vol = Decimal::ZERO;
    let three = Decimal::from_u32(3).context("Failed to create Decimal 3")?;

    for i in 0..data.height() {
        let h = high.get(i);
        let l = low.get(i);
        let c = close.get(i);
        let v = volume.get(i);

        if let (Some(h_val), Some(l_val), Some(c_val), Some(v_val)) = (h, l, c, v) {
            if let (Some(hd), Some(ld), Some(cd), Some(vd)) = (
                Decimal::from_f64(h_val),
                Decimal::from_f64(l_val),
                Decimal::from_f64(c_val),
                Decimal::from_f64(v_val),
            ) {
                let typ_price = (hd + ld + cd) / three;
                cum_vol_price += typ_price * vd;
                cum_vol += vd;

                if cum_vol > Decimal::ZERO {
                    let vwap = cum_vol_price / cum_vol;
                    // To interact with polars series, we need f64 output.
                    if let Some(vwap_f64) = vwap.to_f64() {
                        vwap_values.push(Some(vwap_f64));
                    } else {
                        vwap_values.push(None);
                    }
                } else {
                    vwap_values.push(None);
                }
            } else {
                vwap_values.push(None);
            }
        } else {
            vwap_values.push(None);
        }
    }

    Ok(Series::new("vwap", vwap_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "timestamp" => &[1000i64, 2000i64, 3000i64], // Dummy timestamps for validation
            "high" => &[10.0, 12.0, 14.0],
            "low" => &[8.0, 10.0, 12.0],
            "close" => &[9.0, 11.0, 13.0],
            "volume" => &[100.0, 200.0, 300.0]
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        assert_eq!(out.get(0).unwrap(), 9.0);
        assert!((out.get(1).unwrap() - 10.333333333).abs() < 1e-6);
        assert!((out.get(2).unwrap() - 11.666666666).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_missing = df!("high" => &[1.0])?;
        assert!(calculate(&df_missing).is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "timestamp" => &[1000i64, 2000i64, 3000i64, 4000i64, 5000i64],
            "high" => &[100.0, 102.0, 101.0, 103.0, 102.0],
            "low" => &[98.0, 100.0, 99.0, 101.0, 100.0],
            "close" => &[99.0, 101.0, 100.0, 102.0, 101.0],
            "volume" => &[1000.0, 1500.0, 1200.0, 2000.0, 1800.0]
        )?;
        let result = calculate(&df)?;
        assert_eq!(result.len(), 5);
        Ok(())
    }
}
