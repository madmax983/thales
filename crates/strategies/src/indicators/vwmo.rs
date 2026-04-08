//! vwmo - Volume Weighted Momentum Oscillator
//!
//! The Volume Weighted Momentum Oscillator (VWMO) multiplies price momentum
//! (change in close price over `period`) by volume to confirm trend strength.
//!
//! # Returns
//! Series with indicator values. First `period` values will be null.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::Decimal;
use std::str::FromStr;

/// Calculate Volume Weighted Momentum Oscillator (VWMO)
///
/// # Arguments
/// * `data` - DataFrame with "close" and "volume" columns
/// * `period` - Lookback period for momentum calculation
///
/// # Returns
/// Series with VWMO values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::vwmo;
///
/// fn example() -> anyhow::Result<()> {
///     let df = df!(
///         "close" => &["10", "12", "15"],
///         "volume" => &["100", "200", "300"]
///     )?;
///     let mut df = df.clone();
///     df.try_apply("close", |s| s.cast(&DataType::String))?;
///     df.try_apply("volume", |s| s.cast(&DataType::String))?;
///     let result = vwmo::calculate(&df, 1)?;
///     Ok(())
/// }
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be > 0");
    }

    let close_series = data.column("close").context("Missing 'close' column")?;
    let volume_series = data.column("volume").context("Missing 'volume' column")?;

    let close_arr = close_series.cast(&DataType::String)?;
    let close_chunked = close_arr.str()?;

    let volume_arr = volume_series.cast(&DataType::String)?;
    let volume_chunked = volume_arr.str()?;

    let padding_len = std::cmp::min(data.height(), period);
    let padding = (0..padding_len).map(|_| None);

    let calculated_iter = close_chunked
        .into_iter()
        .skip(period)
        .zip(close_chunked)
        .zip(volume_chunked.into_iter().skip(period))
        .map(|((close_curr_opt, close_prev_opt), vol_curr_opt)| {
            if let (Some(c_curr), Some(c_prev), Some(v_curr)) =
                (close_curr_opt, close_prev_opt, vol_curr_opt)
            {
                if let (Ok(c_curr_dec), Ok(c_prev_dec), Ok(v_curr_dec)) = (
                    Decimal::from_str(c_curr),
                    Decimal::from_str(c_prev),
                    Decimal::from_str(v_curr),
                ) {
                    let momentum = c_curr_dec - c_prev_dec;
                    let vwmo_val = momentum * v_curr_dec;
                    Some(vwmo_val.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        });

    let result_chunked: StringChunked = padding.chain(calculated_iter).collect();
    let mut result_series = result_chunked.into_series();
    result_series.rename("vwmo");

    Ok(result_series)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &["10", "12", "11", "15"],
            "volume" => &["100", "200", "150", "300"]
        )?;

        let result = calculate(&df, 1)?;
        assert_eq!(result.len(), 4);

        let res_arr = result.str()?;
        assert!(res_arr.get(0).is_none());

        if let Some(val) = res_arr.get(1) {
            let val_dec = Decimal::from_str(val).context("parse decimal")?;
            let expected = Decimal::from(400); // (12 - 10) * 200 = 400
            assert_eq!(val_dec, expected);
        } else {
            anyhow::bail!("Expected value at index 1");
        }

        if let Some(val) = res_arr.get(2) {
            let val_dec = Decimal::from_str(val).context("parse decimal")?;
            let expected = Decimal::from(-150); // (11 - 12) * 150 = -150
            assert_eq!(val_dec, expected);
        } else {
            anyhow::bail!("Expected value at index 2");
        }

        if let Some(val) = res_arr.get(3) {
            let val_dec = Decimal::from_str(val).context("parse decimal")?;
            let expected = Decimal::from(1200); // (15 - 11) * 300 = 1200
            assert_eq!(val_dec, expected);
        } else {
            anyhow::bail!("Expected value at index 3");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res = calculate(&df_empty, 1);
        assert!(res.is_err());

        let df_single = df!(
            "close" => &["10"],
            "volume" => &["100"]
        )?;

        let res2 = calculate(&df_single, 1)?;
        assert_eq!(res2.len(), 1);
        assert!(res2.str()?.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &["100", "101", "102", "100", "95", "90", "92"],
            "volume" => &["1000", "1100", "1200", "1500", "2000", "2500", "1800"]
        )?;

        let result = calculate(&df, 2)?;
        assert_eq!(result.len(), 7);

        let res_arr = result.str()?;
        assert!(res_arr.get(0).is_none());
        assert!(res_arr.get(1).is_none());

        if let Some(val) = res_arr.get(2) {
            let val_dec = Decimal::from_str(val).context("parse decimal")?;
            let expected = Decimal::from(2400); // (102.0 - 100.0) = 2.0 * 1200 = 2400
            assert_eq!(val_dec, expected);
        } else {
            anyhow::bail!("Expected value at index 2");
        }

        Ok(())
    }
}
