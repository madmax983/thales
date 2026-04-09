//! ZLEMA - Zero Lag Exponential Moving Average
//!
//! Calculates the Zero Lag Exponential Moving Average (ZLEMA), a variation of the Exponential Moving Average (EMA) that aims to reduce the lag inherent in moving averages.
//! ZLEMA uses adjusted data (original data + its momentum over a specific lag period) before applying the EMA formula.
//!
//! # References
//! - [Investopedia - Zero Lag Exponential Moving Average (ZLEMA)](https://www.investopedia.com/terms/z/zlema.asp)

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Zero Lag Exponential Moving Average (ZLEMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with ZLEMA values. The first `period - 1` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let zlema = strategies::indicators::zlema::calculate(&df, 14)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let close_series = data
        .column("close")
        .context("DataFrame must contain 'close' column")?;
    let close_str = close_series.cast(&DataType::String)?;
    let close_ca = close_str.str()?;

    let lag = (period - 1) / 2;
    let period_dec = Decimal::from_usize(period).context("Invalid period")?;
    let k = Decimal::TWO / (period_dec + Decimal::ONE);

    let mut prev_ema: Option<Decimal> = None;
    let mut window_sum = Decimal::ZERO;
    let mut count = 0;

    let mut zlema_values: Vec<Option<String>> = Vec::with_capacity(close_ca.len());

    let close_vec: Vec<Option<&str>> = close_ca.into_iter().collect();

    for i in 0..close_vec.len() {
        if i < lag {
            zlema_values.push(None);
            continue;
        }

        let val_opt = close_vec[i];
        let lag_val_opt = close_vec[i - lag];

        match (val_opt, lag_val_opt) {
            (Some(val_str), Some(lag_val_str)) => {
                if let (Ok(val_dec), Ok(lag_dec)) =
                    (Decimal::from_str(val_str), Decimal::from_str(lag_val_str))
                {
                    // Adjusted Data = Close + (Close - Close[lag])
                    let adj_data = val_dec + (val_dec - lag_dec);

                    if count < period {
                        window_sum += adj_data;
                        count += 1;

                        if count == period {
                            let seed = window_sum / period_dec;
                            zlema_values.push(Some(seed.to_string()));
                            prev_ema = Some(seed);
                        } else {
                            zlema_values.push(None);
                        }
                    } else {
                        if let Some(prev) = prev_ema {
                            let ema = (adj_data * k) + (prev * (Decimal::ONE - k));
                            zlema_values.push(Some(ema.to_string()));
                            prev_ema = Some(ema);
                        } else {
                            zlema_values.push(None);
                        }
                    }
                } else {
                    zlema_values.push(None);
                    count = 0;
                    window_sum = Decimal::ZERO;
                    prev_ema = None;
                }
            }
            _ => {
                zlema_values.push(None);
                count = 0;
                window_sum = Decimal::ZERO;
                prev_ema = None;
            }
        }
    }

    let s = Series::new("zlema", zlema_values);
    let s = s.cast(&DataType::Float64)?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &["10.0", "11.0", "12.0", "13.0", "14.0", "15.0", "16.0"]
        )?;

        let result = calculate(&df, 3)?;
        let result_str = result.cast(&DataType::String)?;
        let out = result_str.str()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());

        if let Some(val3) = out.get(3) {
            assert_eq!(Decimal::from_str(val3)?, Decimal::from_str("13.0")?);
        } else {
            anyhow::bail!("Missing value at index 3");
        }

        if let Some(val4) = out.get(4) {
            assert_eq!(Decimal::from_str(val4)?, Decimal::from_str("14.0")?);
        } else {
            anyhow::bail!("Missing value at index 4");
        }

        if let Some(val5) = out.get(5) {
            assert_eq!(Decimal::from_str(val5)?, Decimal::from_str("15.0")?);
        } else {
            anyhow::bail!("Missing value at index 5");
        }

        if let Some(val6) = out.get(6) {
            assert_eq!(Decimal::from_str(val6)?, Decimal::from_str("16.0")?);
        } else {
            anyhow::bail!("Missing value at index 6");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        let df_short = df!("close" => &["10.0", "11.0"])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        let res_short_str = res_short.cast(&DataType::String)?;
        assert!(res_short_str.str()?.get(0).is_none());
        assert!(res_short_str.str()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[
                "100.0", "102.0", "101.0", "104.0", "107.0", "105.0", "108.0", "110.0", "109.0", "112.0"
            ]
        )?;

        let result = calculate(&df, 5)?;
        let result_str = result.cast(&DataType::String)?;
        let out = result_str.str()?;

        assert_eq!(out.len(), 10);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());
        assert!(out.get(3).is_none());
        assert!(out.get(4).is_none());
        assert!(out.get(5).is_none());
        assert!(out.get(6).is_some());

        Ok(())
    }
}
