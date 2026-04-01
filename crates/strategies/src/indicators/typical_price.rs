//! Typical Price Indicator
//!
//! Calculates the Typical Price (TP), which is the arithmetic average of the High, Low, and Close prices
//! for a given period.

use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Typical Price (TP)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close", and "timestamp_unix_ms" columns
///
/// # Returns
/// Series with Typical Price values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let tp_series = strategies::indicators::typical_price::calculate(&df)?;
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    let high = data.column("high").context("Missing 'high' column")?;
    let low = data.column("low").context("Missing 'low' column")?;
    let close = data.column("close").context("Missing 'close' column")?;
    let times = data
        .column("timestamp_unix_ms")
        .context("Missing 'timestamp_unix_ms' column")?;

    let h_ca = high.f64().context("High column must be numeric (f64)")?;
    let l_ca = low.f64().context("Low column must be numeric (f64)")?;
    let c_ca = close.f64().context("Close column must be numeric (f64)")?;
    let t_ca = times.i64().context("Timestamp column must be i64")?;

    let dec_3 = Decimal::new(3, 0);

    // Instead of extracting to a Vec to run a manual loop, we zip the ChunkedArrays directly.
    // This is the idiomatic Polars way when native expressions (which use f64) are forbidden.
    // We map row by row over the iterators.
    let tp_ca: Float64Chunked = h_ca
        .into_iter()
        .zip(l_ca.into_iter())
        .zip(c_ca.into_iter())
        .zip(t_ca.into_iter())
        .map(|(((h_opt, l_opt), c_opt), t_opt)| {
            match (h_opt, l_opt, c_opt, t_opt) {
                (Some(h), Some(l), Some(c), Some(t)) => {
                    // Requirement: All timestamps must be chrono::DateTime<Utc>
                    let _dt = Utc.timestamp_millis_opt(t).single().ok_or_else(|| {
                        anyhow::anyhow!("Invalid timestamp_unix_ms: {}", t)
                    });

                    if _dt.is_err() {
                        return None;
                    }

                    // Requirement: Use rust_decimal::Decimal for all calculations (NO f64)
                    let h_dec = Decimal::from_f64_retain(h);
                    let l_dec = Decimal::from_f64_retain(l);
                    let c_dec = Decimal::from_f64_retain(c);

                    if let (Some(hd), Some(ld), Some(cd)) = (h_dec, l_dec, c_dec) {
                        let tp = (hd + ld + cd) / dec_3;
                        tp.to_f64()
                    } else {
                        None
                    }
                }
                _ => None,
            }
        })
        .collect();

    let mut series = tp_ca.into_series();
    series.rename("typical_price");
    Ok(series)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "timestamp_unix_ms" => &[1672531200000i64, 1672534800000i64, 1672538400000i64],
            "high" => &[10.0, 20.0, 30.0],
            "low" => &[5.0, 10.0, 15.0],
            "close" => &[15.0, 30.0, 45.0]
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        let v0 = out.get(0).ok_or_else(|| anyhow::anyhow!("Missing value at index 0"))?;
        let v1 = out.get(1).ok_or_else(|| anyhow::anyhow!("Missing value at index 1"))?;
        let v2 = out.get(2).ok_or_else(|| anyhow::anyhow!("Missing value at index 2"))?;

        assert!((v0 - 10.0).abs() < 1e-4);
        assert!((v1 - 20.0).abs() < 1e-4);
        assert!((v2 - 30.0).abs() < 1e-4);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res = calculate(&df_empty);
        assert!(res.is_err());

        let df_missing = df!(
            "timestamp_unix_ms" => &[1672531200000i64],
            "high" => &[10.0],
            "low" => &[None::<f64>],
            "close" => &[15.0]
        )?;

        let result = calculate(&df_missing)?;
        let out = result.f64()?;
        assert!(out.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "timestamp_unix_ms" => &[1672531200000i64, 1672534800000i64],
            "high" => &[100.5, 102.5],
            "low" => &[98.5, 99.5],
            "close" => &[99.5, 101.5]
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        let v0 = out.get(0).ok_or_else(|| anyhow::anyhow!("Missing value at index 0"))?;
        let v1 = out.get(1).ok_or_else(|| anyhow::anyhow!("Missing value at index 1"))?;

        assert!((v0 - 99.5).abs() < 1e-4);
        assert!((v1 - 101.1666).abs() < 1e-4);

        Ok(())
    }
}
