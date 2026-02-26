//! Keltner Channels - A volatility-based channel indicator using EMA and ATR.

use super::{atr, ema};
use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Keltner Channels
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns.
/// * `ema_period` - Lookback period for the EMA (Middle Band).
/// * `atr_period` - Lookback period for the ATR (Band Width).
/// * `atr_multiplier` - Multiplier for the ATR to determine band width.
///
/// # Returns
/// Tuple of (Lower Band, Middle Band, Upper Band) Series.
///
/// # Example
/// ```rust
/// use strategies::indicators::keltner_channels;
/// use polars::prelude::*;
///
/// // Assuming df is a DataFrame with "high", "low", "close" columns
/// let ema_period = 20;
/// let atr_period = 10;
/// let multiplier = 2.0;
/// // let (lower, middle, upper) = keltner_channels::calculate(&df, ema_period, atr_period, multiplier)?;
/// ```
pub fn calculate(
    data: &DataFrame,
    ema_period: usize,
    atr_period: usize,
    atr_multiplier: f64,
) -> Result<(Series, Series, Series)> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if ema_period == 0 || atr_period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Calculate EMA (Middle Band)
    let ema_series = ema::calculate(data, ema_period).context("Failed to calculate EMA")?;

    // Calculate ATR (Band Width)
    let atr_series = atr::calculate(data, atr_period).context("Failed to calculate ATR")?;

    // Iterate and calculate bands using Decimal for precision
    let mult = Decimal::from_f64_retain(atr_multiplier).unwrap_or(Decimal::ZERO);

    let ema_ca = ema_series.f64()?;
    let atr_ca = atr_series.f64()?;

    let len = data.height();
    let mut upper_vals = Vec::with_capacity(len);
    let mut lower_vals = Vec::with_capacity(len);

    for (e_opt, a_opt) in ema_ca.into_iter().zip(atr_ca.into_iter()) {
        match (e_opt, a_opt) {
            (Some(e), Some(a)) => {
                // Convert f64 to Decimal
                let e_dec = Decimal::from_f64_retain(e);
                let a_dec = Decimal::from_f64_retain(a);

                if let (Some(ed), Some(ad)) = (e_dec, a_dec) {
                    let band_width = ad * mult;
                    let upper = ed + band_width;
                    let lower = ed - band_width;

                    upper_vals.push(upper.to_f64());
                    lower_vals.push(lower.to_f64());
                } else {
                    // Conversion failure (e.g. NaN)
                    upper_vals.push(None);
                    lower_vals.push(None);
                }
            }
            _ => {
                // Missing data
                upper_vals.push(None);
                lower_vals.push(None);
            }
        }
    }

    let lower_series = Series::new("keltner_lower", lower_vals);
    let mut middle_series = ema_series;
    let _ = middle_series.rename("keltner_middle");
    let upper_series = Series::new("keltner_upper", upper_vals);

    Ok((lower_series, middle_series, upper_series))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        // Simple case: Constant price and range.
        // Close = 100. High = 101. Low = 99. TR = 2.
        // EMA(100) -> 100 (after warmup)
        // ATR(2) -> 2 (after warmup)
        // Multiplier = 2.0
        // Upper = 100 + (2 * 2) = 104
        // Lower = 100 - (2 * 2) = 96

        let closes = vec![100.0; 20];
        let highs = vec![101.0; 20];
        let lows = vec![99.0; 20];

        let df = df!(
            "close" => &closes,
            "high" => &highs,
            "low" => &lows
        )?;

        let (lower, middle, upper) = calculate(&df, 5, 5, 2.0)?;

        let l = lower.f64()?;
        let m = middle.f64()?;
        let u = upper.f64()?;

        // Check lengths
        assert_eq!(l.len(), 20);
        assert_eq!(m.len(), 20);
        assert_eq!(u.len(), 20);

        // Check values after warmup (index 10 to be safe)
        if let Some(val) = m.get(10) {
            assert!(
                (val - 100.0).abs() < 1e-10,
                "Middle band mismatch: {} != 100.0",
                val
            );
        }

        if let Some(val) = u.get(10) {
            assert!(
                (val - 104.0).abs() < 1e-10,
                "Upper band mismatch: {} != 104.0",
                val
            );
        }

        if let Some(val) = l.get(10) {
            assert!(
                (val - 96.0).abs() < 1e-10,
                "Lower band mismatch: {} != 96.0",
                val
            );
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res = calculate(&df_empty, 10, 10, 2.0);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Data cannot be empty");

        // Single data point
        let df_single = df!(
            "close" => &[100.0],
            "high" => &[101.0],
            "low" => &[99.0]
        )?;

        let (l, m, u) = calculate(&df_single, 5, 5, 2.0)?;
        assert_eq!(l.len(), 1);
        assert!(l.f64()?.get(0).is_none());
        assert!(m.f64()?.get(0).is_none());
        assert!(u.f64()?.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        // Generate a sine wave price to test movement
        let n = 100;
        let mut closes = Vec::with_capacity(n);
        let mut highs = Vec::with_capacity(n);
        let mut lows = Vec::with_capacity(n);

        for i in 0..n {
            let val = 100.0 + (i as f64 * 0.1).sin() * 10.0;
            closes.push(val);
            highs.push(val + 2.0);
            lows.push(val - 2.0);
        }

        let df = df!(
            "close" => &closes,
            "high" => &highs,
            "low" => &lows
        )?;

        let (lower, middle, upper) = calculate(&df, 14, 14, 2.0)?;

        assert_eq!(lower.len(), n);
        assert_eq!(middle.len(), n);
        assert_eq!(upper.len(), n);

        // Verify bands order: Lower < Middle < Upper (mostly)
        // For valid indices
        let l = lower.f64()?;
        let m = middle.f64()?;
        let u = upper.f64()?;

        for i in 20..n {
            if let (Some(lv), Some(mv), Some(uv)) = (l.get(i), m.get(i), u.get(i)) {
                assert!(lv < mv, "Lower {} not less than Middle {}", lv, mv);
                assert!(mv < uv, "Middle {} not less than Upper {}", mv, uv);
            }
        }

        Ok(())
    }
}
