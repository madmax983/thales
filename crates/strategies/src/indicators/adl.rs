//! Accumulation/Distribution Line (ADL) - Volume-based indicator designed to measure the cumulative flow of money into and out of an asset.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Accumulation/Distribution Line (ADL)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close", "volume" columns
///
/// # Returns
/// Series with ADL values.
///
/// # Example
/// ```rust
/// use anyhow::Result;
/// use polars::prelude::*;
/// use strategies::indicators::adl;
///
/// fn example() -> Result<()> {
///     let df = df!(
///         "high" => &[10.0, 11.0, 12.0],
///         "low" => &[8.0, 9.0, 10.0],
///         "close" => &[9.0, 10.0, 11.0],
///         "volume" => &[100.0, 150.0, 200.0]
///     )?;
///     let result = adl::calculate(&df)?;
///     Ok(())
/// }
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

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

    let volume = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?
        .f64()
        .context("Volume column must be numeric (f64)")?;

    let len = close.len();
    let mut adl_values: Vec<f64> = Vec::with_capacity(len);

    let mut current_adl = Decimal::ZERO;

    for i in 0..len {
        let h = Decimal::from_f64_retain(high.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let l = Decimal::from_f64_retain(low.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let c = Decimal::from_f64_retain(close.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let v =
            Decimal::from_f64_retain(volume.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);

        let high_low_diff = h - l;

        let mfm = if high_low_diff.is_zero() {
            Decimal::ZERO
        } else {
            // Money Flow Multiplier = ((Close - Low) - (High - Close)) / (High - Low)
            ((c - l) - (h - c)) / high_low_diff
        };

        // Money Flow Volume = MFM * Volume
        let mfv = mfm * v;

        current_adl += mfv;
        adl_values.push(current_adl.to_f64().unwrap_or(0.0));
    }

    Ok(Series::new("adl", adl_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_adl_calculation() -> Result<()> {
        // i=0: H=12, L=10, C=11. diff=2. MFM = ((11-10) - (12-11))/2 = (1 - 1)/2 = 0. MFV = 0. V=100. ADL = 0
        // i=1: H=12, L=10, C=12. diff=2. MFM = ((12-10) - (12-12))/2 = (2 - 0)/2 = 1. MFV = 100. V=100. ADL = 100
        // i=2: H=12, L=10, C=10. diff=2. MFM = ((10-10) - (12-10))/2 = (0 - 2)/2 = -1. MFV = -100. V=100. ADL = 0

        let df = df!(
            "high" => &[12.0, 12.0, 12.0],
            "low" => &[10.0, 10.0, 10.0],
            "close" => &[11.0, 12.0, 10.0],
            "volume" => &[100.0, 100.0, 100.0]
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 3);

        // i=0
        let val0 = out.get(0).context("Missing val0")?;
        assert!((val0 - 0.0).abs() < 1e-6);

        // i=1
        let val1 = out.get(1).context("Missing val1")?;
        assert!((val1 - 100.0).abs() < 1e-6);

        // i=2
        let val2 = out.get(2).context("Missing val2")?;
        assert!((val2 - 0.0).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty).is_err());

        // Single row
        let df_short = df!(
            "high" => &[10.0],
            "low" => &[10.0],
            "close" => &[10.0],
            "volume" => &[100.0]
        )?;
        let res = calculate(&df_short)?;
        assert_eq!(res.f64()?.get(0).context("Missing value")?, 0.0);

        // High == Low
        let df_flat = df!(
            "high" => &[10.0, 10.0],
            "low" => &[10.0, 10.0],
            "close" => &[10.0, 10.0],
            "volume" => &[100.0, 100.0]
        )?;
        let res_flat = calculate(&df_flat)?;
        let out_flat = res_flat.f64()?;
        assert_eq!(out_flat.get(1).context("Missing value")?, 0.0);

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "high" => &[10.5, 11.2, 10.8, 11.5, 12.0],
            "low" => &[10.0, 10.5, 10.2, 10.8, 11.0],
            "close" => &[10.2, 11.0, 10.5, 11.2, 11.8],
            "volume" => &[1000.0, 1500.0, 1200.0, 2000.0, 1800.0]
        )?;

        let result = calculate(&df)?;
        assert_eq!(result.len(), 5);
        let _out = result.f64()?;

        Ok(())
    }
}
