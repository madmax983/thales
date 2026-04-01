//! Template Indicator - A placeholder technical indicator
//!
//! # Returns
//! Series with indicator values
//!
//! # Example
//! ```rust
//! use polars::prelude::*;
//! use strategies::indicators::template_indicator;
//!
//! # fn main() -> anyhow::Result<()> {
//! let df = df!(
//!     "close" => &[10, 20, 30],
//!     "timestamp_unix_ms" => &[1622505600000i64, 1622592000000i64, 1622678400000i64]
//! )?;
//! // Ensure close is Float64
//! let mut df = df.clone();
//! df.try_apply("close", |s| s.cast(&DataType::Float64))?;
//!
//! let result = template_indicator::calculate(&df, 2)?;
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Template Indicator
///
/// # Arguments
/// * `data` - DataFrame with OHLCV data
/// * `period` - Lookback period
///
/// # Returns
/// Series with indicator values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::template_indicator;
/// # fn main() -> anyhow::Result<()> {
/// let df = df!(
///     "close" => &[10, 20, 30],
///     "timestamp_unix_ms" => &[1622505600000i64, 1622592000000i64, 1622678400000i64]
/// )?;
/// // Ensure close is Float64
/// let mut df = df.clone();
/// df.try_apply("close", |s| s.cast(&DataType::Float64))?;
///
/// let result = template_indicator::calculate(&df, 2)?;
/// # Ok(())
/// # }
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.is_empty() {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let close_series = data.column("close").context("Missing 'close' column")?.f64()?;

    // Safely parse DataFrame Float64 to Decimal, NO unwrap/expect
    let closes: Vec<Option<Decimal>> = close_series
        .into_iter()
        .map(|v| v.and_then(Decimal::from_f64_retain))
        .collect();

    let mut result_values: Vec<Option<f64>> = vec![None; closes.len()];
    let period_dec = Decimal::from_usize(period).context("Failed to create period Decimal")?;

    let mut sum = Decimal::ZERO;
    let mut count = 0;
    let mut queue: VecDeque<Option<Decimal>> = VecDeque::with_capacity(period + 1);

    for i in 0..closes.len() {
        let val_opt = closes[i];
        queue.push_back(val_opt);

        if let Some(val) = val_opt {
            sum += val;
            count += 1;
        }

        if queue.len() > period {
            if let Some(Some(popped)) = queue.pop_front() {
                sum -= popped;
                count -= 1;
            }
        }

        if queue.len() == period && count == period {
            let val = sum / period_dec;
            // Safely convert back to f64 via to_f64() from rust_decimal::prelude::ToPrimitive
            result_values[i] = val.to_f64();
        }
    }

    Ok(Series::new("template_indicator", result_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10, 20, 30, 40],
            "timestamp_unix_ms" => &[1000i64, 2000i64, 3000i64, 4000i64]
        )?;
        let mut df = df.clone();
        df.try_apply("close", |s| s.cast(&DataType::Float64))?;

        let res = calculate(&df, 2)?;
        let vals = res.f64()?;

        assert!(vals.get(0).is_none());
        assert_eq!(vals.get(1), Some(15.0)); // (10+20)/2
        assert_eq!(vals.get(2), Some(25.0)); // (20+30)/2
        assert_eq!(vals.get(3), Some(35.0)); // (30+40)/2

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res = calculate(&df_empty, 2);
        assert!(res.is_err());
        if let Err(e) = res {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        let df_invalid_period = df!(
            "close" => &[10],
            "timestamp_unix_ms" => &[1000i64]
        )?;
        let mut df_invalid_period = df_invalid_period.clone();
        df_invalid_period.try_apply("close", |s| s.cast(&DataType::Float64))?;

        let res2 = calculate(&df_invalid_period, 0);
        assert!(res2.is_err());

        // Use None for NaN to avoid f64 literal
        let df_nan = df!(
            "close" => &[Some(10), None, Some(30)],
            "timestamp_unix_ms" => &[1000i64, 2000i64, 3000i64]
        )?;
        let mut df_nan = df_nan.clone();
        df_nan.try_apply("close", |s| s.cast(&DataType::Float64))?;

        let res3 = calculate(&df_nan, 2)?;
        let vals3 = res3.f64()?;
        assert!(vals3.get(1).is_none()); // 10.0 + NaN -> invalid
        assert!(vals3.get(2).is_none()); // NaN + 30.0 -> invalid

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut closes = Vec::new();
        let mut timestamps = Vec::new();
        for i in 0..100 {
            closes.push(100 + i);
            timestamps.push(1600000000000i64 + (i as i64 * 86400000));
        }
        let df = df!(
            "close" => closes,
            "timestamp_unix_ms" => timestamps
        )?;
        let mut df = df.clone();
        df.try_apply("close", |s| s.cast(&DataType::Float64))?;

        let res = calculate(&df, 14)?;
        assert_eq!(res.len(), 100);

        let vals = res.f64()?;
        assert!(vals.get(12).is_none());
        assert!(vals.get(13).is_some());

        Ok(())
    }
}
