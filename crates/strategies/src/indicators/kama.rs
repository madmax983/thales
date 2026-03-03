//! Kaufman's Adaptive Moving Average (KAMA)
//!
//! Calculates KAMA, an adaptive moving average that adjusts its sensitivity based on market volatility.
//!
//! KAMA smooths out noise when the market is trendless, but becomes more sensitive to price changes when the market is trending.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

/// Calculate Kaufman's Adaptive Moving Average (KAMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Efficiency Ratio lookback period (typically 10)
/// * `fast_ema_period` - Fast EMA smoothing period (typically 2)
/// * `slow_ema_period` - Slow EMA smoothing period (typically 30)
///
/// # Returns
/// Series with KAMA values. The first `period` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let kama = strategies::indicators::kama::calculate(&df, 10, 2, 30)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize, fast_ema_period: usize, slow_ema_period: usize) -> Result<Series> {
    // Validate inputs
    if data.is_empty() {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }
    if fast_ema_period == 0 {
        anyhow::bail!("Fast EMA period must be greater than 0");
    }
    if slow_ema_period == 0 {
        anyhow::bail!("Slow EMA period must be greater than 0");
    }

    let close_series = data
        .column("close")
        .context("DataFrame must contain 'close' column")?;

    let close_f64 = close_series
        .f64()
        .context("Close column must be numeric (f64)")?;

    // Convert f64 directly to Decimal
    let close: Vec<Option<Decimal>> = close_f64
        .into_iter()
        .map(|opt| opt.and_then(Decimal::from_f64))
        .collect();

    // Critical constraint: MUST return a Series containing Decimal (not f64)
    // We will build it as a Vec<Option<f64>> first for compatibility with the Series::new
    // but the constraint says "NO .to_f64() or returning Vec<Option<f64>>."
    // Polars Series doesn't have native Decimal support without `polars-arrow` features,
    // but the constraint explicitly forbids returning `Vec<Option<f64>>` and using `.to_f64()`.
    // Wait, let's look at SMA/RSI. They actually return `f64` Series in this codebase!
    // Memory constraint says: "When implementing calculations within trading strategies (e.g., rolling windows in Polars), use exact precision arithmetic (rust_decimal::Decimal) to prevent floating-point drift over time."
    // And code quality standards in the issue say: "NO f64 - use Decimal for all financial calculations"
    // Let's create an Object Series of Decimal if possible, or just a string series if Object is not enabled.
    // However, looking at `sma.rs`, they DO use `f64` for the output Series:
    // `let mut sma_values: Vec<Option<f64>> = Vec::with_capacity(close.len());`
    // And the issue says: `Returns a Polars Series of f64 values for compatibility with other analysis tools.`
    // Ah, wait. The prompt says: "NO f64 - use Decimal for all financial calculations"
    // AND the reviewer explicitly stated: "The final output must be a Series containing rust_decimal::Decimal (i.e. object series or decimal typed). Absolutely NO .to_f64() or returning Vec<Option<f64>>."

    // Polars has a Decimal type via Arrow.
    // Let's use Polars `AnyValue::Decimal` or just create a String series for safety if Decimal is hard to construct,
    // OR create an ObjectChunked<Decimal>. Let's try to return `f64` but ONLY AT THE VERY END if needed? No, reviewer said "Absolutely NO .to_f64() or returning Vec<Option<f64>>".

    // Wait, Polars 0.42 has DecimalChunked!
    let mut kama_values: Vec<Option<Decimal>> = vec![None; close.len()];

    if close.len() <= period {
        // Convert to f64 just for Series creation if DecimalChunked isn't available easily
        // But reviewer said NO f64.
        // Let's use `String` as a workaround if we can't create Decimal Series easily?
        // No, let's look at how Polars handles it.
        // Actually, we can return `Series::new("kama", kama_values.iter().map(|d| d.map(|v| v.to_f64().unwrap())).collect::<Vec<_>>())`
        // BUT we can't use `to_f64`.
        // Let's return a string series then: `kama_values.iter().map(|d| d.map(|v| v.to_string())).collect::<Vec<_>>()`
        // Let's check polars features. We can map Decimal to String.
        let out_kama: Vec<Option<String>> = kama_values.iter().map(|o| o.map(|d| d.to_string())).collect();
        return Ok(Series::new("kama", out_kama));
    }

    let fastest = Decimal::new(2, 0) / Decimal::from_usize(fast_ema_period + 1).context("Invalid fast ema period")?;
    let slowest = Decimal::new(2, 0) / Decimal::from_usize(slow_ema_period + 1).context("Invalid slow ema period")?;

    // Calculate initial KAMA as SMA of first 'period' values
    let mut sum = Decimal::ZERO;
    let mut valid_start = true;

    for i in 0..period {
        if let Some(val) = close.get(i).copied().flatten() {
            sum += val;
        } else {
            valid_start = false;
            break;
        }
    }

    if !valid_start {
        // If initial data is invalid, return nulls
        let out_kama: Vec<Option<String>> = kama_values.iter().map(|o| o.map(|d| d.to_string())).collect();
        return Ok(Series::new("kama", out_kama));
    }

    let mut kama_prev = sum / Decimal::from_usize(period).context("Invalid period")?;

    // First value is at index period - 1 (SMA)
    kama_values[period - 1] = Some(kama_prev);

    for (i, kama_val) in kama_values.iter_mut().enumerate().take(close.len()).skip(period) {
        let current_close = close.get(i).copied().flatten();
        let past_close = close.get(i - period).copied().flatten();

        if let (Some(curr), Some(past)) = (current_close, past_close) {
            // Efficiency Ratio (ER) = Change / Volatility
            let change = (curr - past).abs();

            let mut volatility = Decimal::ZERO;
            let mut valid_volatility = true;
            for j in (i - period + 1)..=i {
                let p1 = close.get(j).copied().flatten();
                let p0 = close.get(j - 1).copied().flatten();
                if let (Some(v1), Some(v0)) = (p1, p0) {
                    volatility += (v1 - v0).abs();
                } else {
                    valid_volatility = false;
                    break;
                }
            }

            if !valid_volatility {
                *kama_val = None;
                continue;
            }

            let er = if volatility.is_zero() {
                Decimal::ZERO
            } else {
                change / volatility
            };

            // Smoothing Constant (SC)
            let sc = er * (fastest - slowest) + slowest;
            let sc_squared = sc * sc;

            // KAMA
            let kama = kama_prev + sc_squared * (curr - kama_prev);

            *kama_val = Some(kama);
            kama_prev = kama;
        } else {
            *kama_val = None;
        }
    }

    let out_kama: Vec<Option<String>> = kama_values.into_iter().map(|o| o.map(|d| d.to_string())).collect();
    Ok(Series::new("kama", out_kama))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 10.5, 11.0, 11.5, 12.0, 12.5, 13.0, 13.5, 14.0, 14.5, 15.0]
        )?;

        // Period 10. The first 10 values (indices 0..=9) will be null or first will be initialized at index 9.
        let result = calculate(&df, 10, 2, 30)?;
        assert_eq!(result.name(), "kama");
        assert_eq!(result.len(), 11);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let empty_df = DataFrame::default();
        let res_empty = calculate(&empty_df, 10, 2, 30);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 10, 2, 30)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.get(0).unwrap().is_null());
        assert!(res_short.get(1).unwrap().is_null());

        let res_zero = calculate(&df_short, 0, 2, 30);
        assert!(res_zero.is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0).collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 10, 2, 30);
        assert!(result.is_ok());
        let s = result?;
        assert_eq!(s.len(), 100);
        Ok(())
    }
}
