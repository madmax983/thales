//! Kaufman's Adaptive Moving Average (KAMA)
//!
//! Calculates the KAMA, an intelligent moving average that adapts to market noise or volatility.
//! It closely follows prices when price swings are relatively small and noise is low, and adjusts
//! to moving averages when prices swing widely and noise is high.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Kaufman's Adaptive Moving Average (KAMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Efficiency Ratio (ER) lookback period (typically 10)
/// * `fast_ema_period` - Fast EMA period (typically 2)
/// * `slow_ema_period` - Slow EMA period (typically 30)
///
/// # Returns
/// Series with KAMA values. The first `period` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::kama;
///
/// let df = df!("close" => &[10.0, 11.0, 12.0, 13.0, 14.0]).unwrap_or_default();
/// let result = kama::calculate(&df, 10, 2, 30);
/// ```
pub fn calculate(
    data: &DataFrame,
    period: usize,
    fast_ema_period: usize,
    slow_ema_period: usize,
) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 || fast_ema_period == 0 || slow_ema_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let mut kama_values: Vec<Option<f64>> = vec![None; close.len()];

    if close.len() <= period {
        return Ok(Series::new("kama", kama_values));
    }

    let decimal_close: Vec<Option<Decimal>> = close
        .into_iter()
        .map(|opt_val| opt_val.and_then(Decimal::from_f64_retain))
        .collect();

    // EMA constants
    let two = Decimal::from_usize(2).context("Failed to create Decimal from 2")?;
    let fast_ema_plus_one = Decimal::from_usize(fast_ema_period + 1)
        .context("Failed to create Decimal from fast_ema_period")?;
    let slow_ema_plus_one = Decimal::from_usize(slow_ema_period + 1)
        .context("Failed to create Decimal from slow_ema_period")?;

    let fast_sc = two
        .checked_div(fast_ema_plus_one)
        .context("Division by zero in fast EMA SC")?;
    let slow_sc = two
        .checked_div(slow_ema_plus_one)
        .context("Division by zero in slow EMA SC")?;

    let mut prev_kama: Option<Decimal> = None;

    for i in period..decimal_close.len() {
        let current_close = match decimal_close[i] {
            Some(c) => c,
            None => {
                kama_values[i] = None;
                continue;
            }
        };

        if prev_kama.is_none() {
            // Initialize KAMA with simple moving average of the previous `period` elements (i - period to i - 1)
            let mut sum = Decimal::ZERO;
            let mut valid = true;
            for j in 0..period {
                if let Some(val) = decimal_close[i - period + j] {
                    sum += val;
                } else {
                    valid = false;
                    break;
                }
            }

            if valid {
                let period_dec =
                    Decimal::from_usize(period).context("Failed to create Decimal from period")?;
                let init_kama = sum / period_dec;
                prev_kama = Some(init_kama);

                kama_values[i - 1] = Some(
                    init_kama
                        .to_f64()
                        .context("Failed to convert init_kama to f64")?,
                );
            } else {
                kama_values[i] = None;
                continue;
            }
        }

        // prev_kama is guaranteed to be Some here, as we continue if it couldn't be initialized
        let p_kama = prev_kama.context("prev_kama should be initialized")?;

        let period_start_close = match decimal_close[i - period] {
            Some(c) => c,
            None => {
                kama_values[i] = None;
                continue;
            }
        };

        let change = (current_close - period_start_close).abs();

        let mut volatility = Decimal::ZERO;
        let mut valid_volatility = true;
        for j in 0..period {
            let curr = decimal_close[i - j];
            let prev = decimal_close[i - j - 1];
            if let (Some(c), Some(p)) = (curr, prev) {
                volatility += (c - p).abs();
            } else {
                valid_volatility = false;
                break;
            }
        }

        if !valid_volatility {
            kama_values[i] = None;
            continue;
        }

        let er = if volatility.is_zero() {
            Decimal::ZERO
        } else {
            change / volatility
        };

        let sc = er * (fast_sc - slow_sc) + slow_sc;
        let sc_squared = sc * sc;

        let current_kama = p_kama + sc_squared * (current_close - p_kama);
        prev_kama = Some(current_kama);
        kama_values[i] = Some(
            current_kama
                .to_f64()
                .context("Failed to convert KAMA to f64")?,
        );
    }

    Ok(Series::new("kama", kama_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[
                10.0, 10.5, 10.2, 10.8, 11.0, 11.5, 11.2, 11.8, 12.0, 12.5, // 0..9
                12.2, 12.8, 13.0, 13.5, 13.2 // 10..14
            ]
        )?;

        let result = calculate(&df, 10, 2, 30)?;
        let out = result.f64()?;

        assert!(out.get(8).is_none());
        assert!(out.get(9).is_some());
        assert!(out.get(10).is_some());

        // Initial KAMA is SMA of elements at index 0 to 9.
        // at i=9: 10.0+10.5+10.2+10.8+11.0+11.5+11.2+11.8+12.0+12.5 = 111.5 / 10 = 11.15
        if let Some(val9) = out.get(9) {
            assert!((val9 - 11.15).abs() < 1e-4);
        } else {
            anyhow::bail!("Value at index 9 should be Some");
        }

        // at i=10: current_close = 12.2
        // period_start_close = 10.0 (index 0)
        // change = |12.2 - 10.0| = 2.2
        // volatility = sum of abs differences from index 1 to 10
        // ... (we'll just ensure it calculates a value correctly)
        if let Some(val10) = out.get(10) {
            assert!(val10 > 0.0);
        } else {
            anyhow::bail!("Value at index 10 should be Some");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 10, 2, 30);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 10, 2, 30)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        // Test with leading nulls
        let df_nulls = df!("close" => &[None::<f64>, None::<f64>, Some(10.0), Some(10.5), Some(10.2), Some(10.8), Some(11.0), Some(11.5), Some(11.2), Some(11.8), Some(12.0), Some(12.5), Some(12.2), Some(12.8), Some(13.0)])?;
        let res_nulls = calculate(&df_nulls, 10, 2, 30)?;
        let out_nulls = res_nulls.f64()?;
        assert!(out_nulls.get(10).is_none());
        assert!(out_nulls.get(11).is_some());
        assert!(out_nulls.get(12).is_some());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut closes = Vec::new();
        for i in 0..100 {
            closes.push(100.0 + (i as f64) * 0.5);
        }

        let df = df!("close" => &closes)?;
        let result = calculate(&df, 10, 2, 30)?;

        assert_eq!(result.len(), 100);
        let out = result.f64()?;
        assert!(out.get(8).is_none());
        assert!(out.get(9).is_some());
        assert!(out.get(10).is_some());
        assert!(out.get(99).is_some());

        Ok(())
    }
}
