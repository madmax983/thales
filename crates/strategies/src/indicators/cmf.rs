//! Chaikin Money Flow (CMF) - Volume-weighted average of accumulation and distribution
//! over a specified period.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Chaikin Money Flow (CMF)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close", "volume" columns
/// * `period` - Lookback period (typically 20 or 21)
///
/// # Returns
/// Series with CMF values. The first `period - 1` values will be null.
///
/// # Example
/// ```rust
/// use strategies::indicators::cmf;
/// use polars::prelude::*;
/// // Assuming df is a DataFrame with "high", "low", "close", "volume" columns
/// // let cmf_series = cmf::calculate(&df, 20)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
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
    let mut cmf_values: Vec<Option<f64>> = vec![None; len];

    if len < period {
        return Ok(Series::new("cmf", cmf_values));
    }

    let mut mfv_values = Vec::with_capacity(len);
    let mut vol_values = Vec::with_capacity(len);

    for i in 0..len {
        let h = Decimal::from_f64_retain(high.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let l = Decimal::from_f64_retain(low.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let c = Decimal::from_f64_retain(close.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let v =
            Decimal::from_f64_retain(volume.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);

        let high_low_diff = h - l;

        let multiplier = if high_low_diff.is_zero() {
            Decimal::ZERO
        } else {
            let num = (c - l) - (h - c);
            num.checked_div(high_low_diff).unwrap_or(Decimal::ZERO)
        };

        let mfv = multiplier * v;

        mfv_values.push(mfv);
        vol_values.push(v);
    }

    // Calculate initial sums for the first window
    let mut sum_mfv = Decimal::ZERO;
    let mut sum_vol = Decimal::ZERO;

    for i in 0..period {
        sum_mfv += mfv_values[i];
        sum_vol += vol_values[i];
    }

    // Calculate CMF for the first complete window (index `period - 1`)
    if !sum_vol.is_zero() {
        let cmf = sum_mfv.checked_div(sum_vol).unwrap_or(Decimal::ZERO);
        cmf_values[period - 1] = Some(cmf.to_f64().unwrap_or(0.0));
    } else {
        cmf_values[period - 1] = Some(0.0);
    }

    // Slide the window
    for i in period..len {
        // Add new
        sum_mfv += mfv_values[i];
        sum_vol += vol_values[i];

        // Remove old
        sum_mfv -= mfv_values[i - period];
        sum_vol -= vol_values[i - period];

        if !sum_vol.is_zero() {
            let cmf = sum_mfv.checked_div(sum_vol).unwrap_or(Decimal::ZERO);
            cmf_values[i] = Some(cmf.to_f64().unwrap_or(0.0));
        } else {
            cmf_values[i] = Some(0.0);
        }
    }

    Ok(Series::new("cmf", cmf_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_cmf_calculation() -> Result<()> {
        // Known values calculation
        // Multiplier = ((C - L) - (H - C)) / (H - L)
        // MFV = Multiplier * Volume
        // CMF = sum(MFV, period) / sum(Volume, period)

        // Data:
        // i=0: H=10, L=8, C=9, V=100
        //   Multiplier = ((9-8) - (10-9)) / (10-8) = (1 - 1) / 2 = 0
        //   MFV = 0 * 100 = 0
        // i=1: H=12, L=10, C=11.5, V=200
        //   Multiplier = ((11.5-10) - (12-11.5)) / (12-10) = (1.5 - 0.5) / 2 = 1 / 2 = 0.5
        //   MFV = 0.5 * 200 = 100
        // i=2: H=11, L=9, C=9.5, V=150
        //   Multiplier = ((9.5-9) - (11-9.5)) / (11-9) = (0.5 - 1.5) / 2 = -1 / 2 = -0.5
        //   MFV = -0.5 * 150 = -75
        // i=3: H=10, L=9, C=10, V=100
        //   Multiplier = ((10-9) - (10-10)) / (10-9) = (1 - 0) / 1 = 1
        //   MFV = 1 * 100 = 100

        // Period = 2
        // i=0: None
        // i=1 (window 0, 1): sum(MFV) = 0 + 100 = 100. sum(V) = 100 + 200 = 300. CMF = 100 / 300 = 0.3333
        // i=2 (window 1, 2): sum(MFV) = 100 - 75 = 25. sum(V) = 200 + 150 = 350. CMF = 25 / 350 = 0.0714
        // i=3 (window 2, 3): sum(MFV) = -75 + 100 = 25. sum(V) = 150 + 100 = 250. CMF = 25 / 250 = 0.1

        let df = df!(
            "high" => &[10.0, 12.0, 11.0, 10.0],
            "low" => &[8.0, 10.0, 9.0, 9.0],
            "close" => &[9.0, 11.5, 9.5, 10.0],
            "volume" => &[100.0, 200.0, 150.0, 100.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 4);
        assert!(out.get(0).is_none());

        // i=1
        let val1 = out.get(1).unwrap();
        assert!(
            (val1 - 0.333333).abs() < 1e-5,
            "Expected ~0.333333, got {}",
            val1
        );

        // i=2
        let val2 = out.get(2).unwrap();
        assert!(
            (val2 - 0.071428).abs() < 1e-5,
            "Expected ~0.071428, got {}",
            val2
        );

        // i=3
        let val3 = out.get(3).unwrap();
        assert!((val3 - 0.1).abs() < 1e-5, "Expected 0.1, got {}", val3);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty, 14).is_err());

        let df_short = df!(
            "high" => &[10.0],
            "low" => &[8.0],
            "close" => &[9.0],
            "volume" => &[100.0]
        )?;
        let res = calculate(&df_short, 5)?;
        assert!(res.f64()?.get(0).is_none());

        let df_zero_high_low_diff = df!(
            "high" => &[10.0, 10.0, 10.0],
            "low" => &[10.0, 10.0, 10.0],
            "close" => &[10.0, 10.0, 10.0],
            "volume" => &[100.0, 100.0, 100.0]
        )?;
        // When High == Low, multiplier should be 0 to avoid division by zero
        let res = calculate(&df_zero_high_low_diff, 2)?;
        let out = res.f64()?;
        assert!(out.get(0).is_none());
        assert_eq!(out.get(1).unwrap(), 0.0);
        assert_eq!(out.get(2).unwrap(), 0.0);

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        // Just verify it doesn't panic on a larger dataset
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut closes = Vec::new();
        let mut vols = Vec::new();

        for i in 0..100 {
            let base = 100.0 + (i as f64) * 0.1;
            highs.push(base + 2.0);
            lows.push(base - 2.0);
            closes.push(base);
            vols.push(1000.0 + (i as f64) * 10.0);
        }

        let df = df!(
            "high" => &highs,
            "low" => &lows,
            "close" => &closes,
            "volume" => &vols
        )?;

        let result = calculate(&df, 20)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 100);
        assert!(out.get(18).is_none());
        assert!(out.get(19).is_some());
        assert!(out.get(99).is_some());

        Ok(())
    }
}
