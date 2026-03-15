//! Chaikin Oscillator - Measures the momentum of the Accumulation/Distribution Line using the MACD formula.

use super::ema;
use super::adl;
use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;

/// Calculate Chaikin Oscillator
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close", "volume" columns
/// * `fast_period` - Fast EMA period (typically 3)
/// * `slow_period` - Slow EMA period (typically 10)
///
/// # Returns
/// Series with Chaikin Oscillator values.
///
/// # Example
/// ```rust
/// use anyhow::Result;
/// use polars::prelude::*;
/// use strategies::indicators::chaikin_oscillator;
///
/// fn example() -> Result<()> {
///     let df = df!(
///         "high" => &[10.0, 11.0, 12.0],
///         "low" => &[8.0, 9.0, 10.0],
///         "close" => &[9.0, 10.0, 11.0],
///         "volume" => &[100.0, 150.0, 200.0]
///     )?;
///     // Note: period 3 and 10 usually require more data.
///     let result = chaikin_oscillator::calculate(&df, 3, 10)?;
///     Ok(())
/// }
/// ```
pub fn calculate(
    data: &DataFrame,
    fast_period: usize,
    slow_period: usize,
) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if fast_period >= slow_period {
        anyhow::bail!("fast_period must be less than slow_period");
    }

    // Step 1: Calculate ADL
    let adl_series = adl::calculate(data).context("Failed to calculate ADL")?;

    // We need to wrap ADL in a DataFrame with "close" column for EMA function
    let mut adl_df = DataFrame::new(vec![adl_series])?;
    adl_df.rename("adl", "close")?;

    // Step 2: Calculate Fast and Slow EMAs of ADL
    let fast_ema_series = ema::calculate(&adl_df, fast_period)
        .context("Failed to calculate Fast EMA of ADL")?;
    let slow_ema_series = ema::calculate(&adl_df, slow_period)
        .context("Failed to calculate Slow EMA of ADL")?;

    let fast_ema = fast_ema_series.f64()?;
    let slow_ema = slow_ema_series.f64()?;

    let len = data.height();
    let mut chaikin_values: Vec<Option<f64>> = Vec::with_capacity(len);

    for i in 0..len {
        let fast_val = fast_ema.get(i);
        let slow_val = slow_ema.get(i);

        match (fast_val, slow_val) {
            (Some(f), Some(s)) if !s.is_nan() && !f.is_nan() => {
                let f_dec = Decimal::from_f64_retain(f).unwrap_or(Decimal::ZERO);
                let s_dec = Decimal::from_f64_retain(s).unwrap_or(Decimal::ZERO);

                // Chaikin Oscillator = Fast EMA of ADL - Slow EMA of ADL
                let osc = f_dec - s_dec;
                chaikin_values.push(osc.to_f64());
            }
            _ => chaikin_values.push(None),
        }
    }

    let result_series = Series::new("chaikin_oscillator", chaikin_values);
    Ok(result_series)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_chaikin_oscillator_calculation() -> Result<()> {
        // Values trace for ADL:
        // i=0: H=12, L=10, C=11. diff=2. MFM = 0. MFV = 0. ADL = 0
        // i=1: H=12, L=10, C=12. diff=2. MFM = 1. MFV = 100. ADL = 100
        // i=2: H=12, L=10, C=10. diff=2. MFM = -1. MFV = -100. ADL = 0
        // i=3: H=12, L=10, C=11. diff=2. MFM = 0. MFV = 0. ADL = 0
        // i=4: H=12, L=10, C=12. diff=2. MFM = 1. MFV = 100. ADL = 100
        // i=5: H=12, L=10, C=10. diff=2. MFM = -1. MFV = -100. ADL = 0
        // Fast EMA (2):
        // i=0: None (ADL 0)
        // i=1: seed SMA(0, 100) = 50.0
        // i=2: EMA(0) = (0 * 2/3) + (50.0 * 1/3) = 16.666666
        // i=3: EMA(0) = (0 * 2/3) + (16.666666 * 1/3) = 5.555555
        // i=4: EMA(100) = (100 * 2/3) + (5.555555 * 1/3) = 66.666666 + 1.851851 = 68.518518
        // i=5: EMA(0) = (0 * 2/3) + (68.518518 * 1/3) = 22.839506

        // Slow EMA (4):
        // i=0: None
        // i=1: None
        // i=2: None
        // i=3: seed SMA(0, 100, 0, 0) = 25.0
        // i=4: EMA(100) = (100 * 2/5) + (25.0 * 3/5) = 40.0 + 15.0 = 55.0
        // i=5: EMA(0) = (0 * 2/5) + (55.0 * 3/5) = 0.0 + 33.0 = 33.0

        // Chaikin Oscillator (Fast EMA - Slow EMA):
        // i=3: 5.555555 - 25.0 = -19.444444
        // i=4: 68.518518 - 55.0 = 13.518518
        // i=5: 22.839506 - 33.0 = -10.160493

        let df = df!(
            "high" => &[12.0, 12.0, 12.0, 12.0, 12.0, 12.0],
            "low" => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0],
            "close" => &[11.0, 12.0, 10.0, 11.0, 12.0, 10.0],
            "volume" => &[100.0, 100.0, 100.0, 100.0, 100.0, 100.0]
        )?;

        let result = calculate(&df, 2, 4)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 6);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());

        let val3 = out.get(3).context("Missing val3")?;
        assert!((val3 - (-19.444444)).abs() < 1e-4);

        let val4 = out.get(4).context("Missing val4")?;
        assert!((val4 - 13.518518).abs() < 1e-4);

        let val5 = out.get(5).context("Missing val5")?;
        assert!((val5 - (-10.160493)).abs() < 1e-4);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty, 3, 10).is_err());

        // Invalid params
        let df_short = df!(
            "high" => &[10.0],
            "low" => &[10.0],
            "close" => &[10.0],
            "volume" => &[100.0]
        )?;
        assert!(calculate(&df_short, 10, 3).is_err()); // fast >= slow

        // Single row
        let res = calculate(&df_short, 3, 10)?;
        assert!(res.f64()?.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "high" => &[10.5, 11.2, 10.8, 11.5, 12.0, 12.5, 12.2, 11.8, 11.5, 11.0, 11.2, 11.5, 12.0],
            "low" => &[10.0, 10.5, 10.2, 10.8, 11.0, 11.8, 11.5, 11.0, 10.5, 10.0, 10.2, 10.5, 11.0],
            "close" => &[10.2, 11.0, 10.5, 11.2, 11.8, 12.2, 11.8, 11.2, 10.8, 10.5, 10.8, 11.2, 11.5],
            "volume" => &[1000.0, 1500.0, 1200.0, 2000.0, 1800.0, 2200.0, 2000.0, 1800.0, 1500.0, 1200.0, 1300.0, 1500.0, 1800.0]
        )?;

        let result = calculate(&df, 3, 10)?;
        assert_eq!(result.len(), 13);

        Ok(())
    }
}
