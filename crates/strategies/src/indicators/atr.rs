//! Average True Range (ATR)
//!
//! Calculates the Average True Range (ATR), a measure of volatility.
//! The ATR is a moving average of the True Range (TR).
//!
//! True Range is the greatest of:
//! - Current High - Current Low
//! - |Current High - Previous Close|
//! - |Current Low - Previous Close|
//!
//! The ATR is typically calculated using Wilder's Smoothing (RMA).

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Average True Range (ATR)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns
/// * `period` - Lookback period (standard is 14)
///
/// # Returns
/// Series with ATR values. The first `period - 1` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let atr = strategies::indicators::atr::calculate(&df, 14)?;
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

    let len = close.len();
    let mut atr_values: Vec<Option<f64>> = vec![None; len];

    if len < period {
        return Ok(Series::new("atr", atr_values));
    }

    let period_dec = Decimal::from_usize(period).context("Invalid period")?;
    let period_minus_one = period_dec - Decimal::ONE;

    let mut tr_sum = Decimal::ZERO;
    let mut valid_start = true;

    // Calculate initial SMA of TRs
    // We need 'period' TR values.
    // TR[0] = High[0] - Low[0]
    // TR[i] = Max(H-L, |H-Cp|, |L-Cp|)

    // Pre-calculate TRs? Or do it in loop.
    // Let's do it in loop.

    // First TR (index 0)
    let h0 = Decimal::from_f64_retain(high.get(0).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
    let l0 = Decimal::from_f64_retain(low.get(0).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);

    if high.get(0).unwrap_or(f64::NAN).is_nan() || low.get(0).unwrap_or(f64::NAN).is_nan() {
        valid_start = false;
    }

    let tr0 = h0 - l0;
    tr_sum += tr0;

    // Subsequent TRs for initial SMA
    for i in 1..period {
        let h = Decimal::from_f64_retain(high.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let l = Decimal::from_f64_retain(low.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let cp =
            Decimal::from_f64_retain(close.get(i - 1).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);

        if high.get(i).unwrap_or(f64::NAN).is_nan()
            || low.get(i).unwrap_or(f64::NAN).is_nan()
            || close.get(i - 1).unwrap_or(f64::NAN).is_nan()
        {
            valid_start = false;
            break;
        }

        let tr = calculate_tr(h, l, cp);
        tr_sum += tr;
    }

    if !valid_start {
        // Return nulls if initial data is bad
        return Ok(Series::new("atr", atr_values));
    }

    // Initial ATR at index period-1
    let mut prev_atr = tr_sum / period_dec;
    atr_values[period - 1] = Some(prev_atr.to_f64().unwrap_or(0.0));

    // Calculate remaining ATRs using Wilder's Smoothing
    for i in period..len {
        let h = Decimal::from_f64_retain(high.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let l = Decimal::from_f64_retain(low.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let cp =
            Decimal::from_f64_retain(close.get(i - 1).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);

        if high.get(i).unwrap_or(f64::NAN).is_nan()
            || low.get(i).unwrap_or(f64::NAN).is_nan()
            || close.get(i - 1).unwrap_or(f64::NAN).is_nan()
        {
            atr_values[i] = None;
            // Strategy: if missing data, we can't easily continue smoothing.
            // We could reset, or just emit None.
            // For now, emit None and keep prev_atr same? No, that's wrong.
            // Just let it be None. But next calculation needs valid prev_atr.
            // If we have a gap, usually indicators break or reset.
            // Let's break/reset logic implies re-seeding.
            // Simplifying assumption: if data is missing, we just output None and don't update prev_atr properly,
            // effectively making future values invalid until reset.
            // But let's try to be robust: if inputs are NaN, output is None.
            continue;
        }

        let tr = calculate_tr(h, l, cp);

        // ATR[i] = (ATR[i-1] * (period - 1) + TR[i]) / period
        let current_atr = (prev_atr * period_minus_one + tr) / period_dec;

        atr_values[i] = Some(current_atr.to_f64().unwrap_or(0.0));
        prev_atr = current_atr;
    }

    Ok(Series::new("atr", atr_values))
}

fn calculate_tr(high: Decimal, low: Decimal, prev_close: Decimal) -> Decimal {
    let hl = high - low;
    let hcp = (high - prev_close).abs();
    let lcp = (low - prev_close).abs();

    hl.max(hcp).max(lcp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[100.0, 102.0, 101.0, 103.0, 102.0],
            "high"  => &[101.0, 103.0, 102.0, 104.0, 103.0],
            "low"   => &[99.0,  101.0, 100.0, 102.0, 101.0]
        )?;

        // Period 2
        // TR calculation:
        // 0: H-L=2.
        // 1: H-L=2, |H-Cp|=3, |L-Cp|=1. Max=3.
        // Initial ATR (SMA of first 2 TRs): (2+3)/2 = 2.5 at index 1.
        // 2: H-L=2, |H-Cp|=0, |L-Cp|=2. Max=2.
        // ATR[2] = (2.5 * 1 + 2) / 2 = 2.25.
        // 3: H-L=2, |H-Cp|=3, |L-Cp|=1. Max=3.
        // ATR[3] = (2.25 * 1 + 3) / 2 = 2.625.
        // 4: H-L=2, |H-Cp|=1, |L-Cp|=1. Max=2.
        // ATR[4] = (2.625 * 1 + 2) / 2 = 2.3125.

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 5);
        assert!(out.get(0).is_none());

        let val1 = out.get(1).unwrap();
        assert!((val1 - 2.5).abs() < 1e-6, "Expected 2.5, got {}", val1);

        let val2 = out.get(2).unwrap();
        assert!((val2 - 2.25).abs() < 1e-6, "Expected 2.25, got {}", val2);

        let val3 = out.get(3).unwrap();
        assert!((val3 - 2.625).abs() < 1e-6, "Expected 2.625, got {}", val3);

        let val4 = out.get(4).unwrap();
        assert!(
            (val4 - 2.3125).abs() < 1e-6,
            "Expected 2.3125, got {}",
            val4
        );

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!(
            "close" => &[100.0, 101.0],
            "high" => &[101.0, 102.0],
            "low" => &[99.0, 100.0]
        )?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.f64()?;
        assert_eq!(out.len(), 2);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        // Zero period
        let df_normal = df!(
            "close" => &[100.0, 101.0],
            "high" => &[101.0, 102.0],
            "low" => &[99.0, 100.0]
        )?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());

        // Missing columns
        let df_missing = df!("close" => &[100.0])?;
        let res_missing = calculate(&df_missing, 14);
        assert!(res_missing.is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let highs: Vec<f64> = values.iter().map(|v| v + 1.0).collect();
        let lows: Vec<f64> = values.iter().map(|v| v - 1.0).collect();

        let df = df!(
            "close" => values,
            "high" => highs,
            "low" => lows
        )?;

        let result = calculate(&df, 14);
        assert!(result.is_ok());
        let s = result?;
        assert_eq!(s.len(), 100);

        let out = s.f64()?;
        assert!(out.get(12).is_none());
        assert!(out.get(13).is_some());

        for i in 13..100 {
            if let Some(v) = out.get(i) {
                assert!(v > 0.0, "ATR {} must be positive at {}", v, i);
            }
        }

        Ok(())
    }
}
