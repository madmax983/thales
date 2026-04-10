//! Custom Aroon Indicator
//!
//! An implementation of the Aroon Indicator using pure Decimal math.
//!
//! # Returns
//! Tuple of (Series Aroon Up, Series Aroon Down)
//!
//! # Example
//! ```rust
//! use polars::prelude::*;
//! use anyhow::Result;
//! use strategies::indicators::aroon_custom;
//!
//! fn main() -> Result<()> {
//! let df = df!(
//!     "high" => &["10", "15", "20", "10"],
//!     "low" => &["5", "10", "15", "5"],
//!     "close" => &["8", "12", "18", "8"],
//!     "timestamp_unix_ms" => &[1600000000000i64, 1600086400000i64, 1600172800000i64, 1600259200000i64]
//! )?;
//!
//! let (up, down) = aroon_custom::calculate(&df, 2)?;
//! Ok(())
//! }
//! ```

use anyhow::Result;
use polars::prelude::*;
use rust_decimal::Decimal;

use std::str::FromStr;

/// Calculate Custom Aroon
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", and "timestamp_unix_ms" columns (as strings or castable to strings)
/// * `period` - Lookback period
///
/// # Returns
/// Series with indicator values
pub fn calculate(data: &DataFrame, period: usize) -> Result<(Series, Series)> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be > 0");
    }

    let high_series = data.column("high")?.cast(&DataType::String)?;
    let low_series = data.column("low")?.cast(&DataType::String)?;

    let high_chunked = high_series.str()?;
    let low_chunked = low_series.str()?;

    let mut up_vals: Vec<Option<String>> = vec![None; data.height()];
    let mut down_vals: Vec<Option<String>> = vec![None; data.height()];

    let mut highs: Vec<Option<Decimal>> = Vec::with_capacity(data.height());
    let mut lows: Vec<Option<Decimal>> = Vec::with_capacity(data.height());

    for (h, l) in high_chunked.into_iter().zip(low_chunked.into_iter()) {
        highs.push(h.map(|s| Decimal::from_str(s).unwrap_or(Decimal::ZERO)));
        lows.push(l.map(|s| Decimal::from_str(s).unwrap_or(Decimal::ZERO)));
    }

    let period_dec = Decimal::from(period);
    let hundred = Decimal::from(100);

    for i in 0..data.height() {
        if i < period {
            continue;
        }

        let mut highest_high = Decimal::MIN;
        let mut lowest_low = Decimal::MAX;
        let mut highest_index = 0;
        let mut lowest_index = 0;

        for j in (i - period)..=i {
            if let Some(h) = highs[j] {
                if h > highest_high {
                    highest_high = h;
                    highest_index = j;
                }
            }
            if let Some(l) = lows[j] {
                if l < lowest_low {
                    lowest_low = l;
                    lowest_index = j;
                }
            }
        }

        let days_since_high = Decimal::from(i - highest_index);
        let days_since_low = Decimal::from(i - lowest_index);

        let aroon_up = ((period_dec - days_since_high) / period_dec) * hundred;
        let aroon_down = ((period_dec - days_since_low) / period_dec) * hundred;

        up_vals[i] = Some(aroon_up.to_string());
        down_vals[i] = Some(aroon_down.to_string());
    }

    let up_series = Series::new("aroon_up", up_vals);
    let down_series = Series::new("aroon_down", down_vals);

    Ok((up_series, down_series))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" => &["10", "12", "15", "14", "13", "16"],
            "low" => &["5", "6", "8", "7", "6", "9"],
            "timestamp_unix_ms" => &[1000i64, 2000, 3000, 4000, 5000, 6000]
        )?;

        let (up, _down) = calculate(&df, 3)?;

        let up_str = up.str()?;

        if let Some(val) = up_str.get(3) {
            assert!(val.starts_with("66.66"));
        } else {
            anyhow::bail!("Missing value");
        }

        if let Some(val) = up_str.get(4) {
            assert!(val.starts_with("33.33"));
        } else {
            anyhow::bail!("Missing value");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res = calculate(&df_empty, 3);
        if res.is_ok() {
            anyhow::bail!("Should fail on empty data");
        }

        let df_single = df!(
            "high" => &["10"],
            "low" => &["5"],
            "timestamp_unix_ms" => &[1000i64]
        )?;
        let (up, _down) = calculate(&df_single, 3)?;
        assert_eq!(up.len(), 1);
        assert_eq!(up.str()?.get(0), None);

        let res2 = calculate(&df_single, 0);
        if res2.is_ok() {
            anyhow::bail!("Should fail on period 0");
        }

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut times = Vec::new();

        for i in 0..50 {
            highs.push(format!("{}", 100 + i));
            lows.push(format!("{}", 90 + i));
            times.push(i as i64 * 1000);
        }

        let df = df!(
            "high" => highs,
            "low" => lows,
            "timestamp_unix_ms" => times
        )?;

        let (up, _down) = calculate(&df, 14)?;
        assert_eq!(up.len(), 50);

        let up_str = up.str()?;
        if let Some(val) = up_str.get(20) {
            assert_eq!(val, "100");
        } else {
            anyhow::bail!("Missing value");
        }

        Ok(())
    }
}
