//! hull_moving_average - Hull Moving Average (HMA)
//!
//! The Hull Moving Average reduces the lag of a traditional moving average, while retaining its smoothness.
//! HMA = WMA(2 * WMA(n/2) - WMA(n), sqrt(n))

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Internal helper for Weighted Moving Average (WMA) using strict Decimal calculations.
///
/// Returns a Vec<Option<Decimal>> instead of a Series to stay entirely in Decimal space
/// for intermediate steps without losing precision or converting to f64.
fn calculate_wma_decimal(close: &[Option<Decimal>], period: usize) -> Result<Vec<Option<Decimal>>> {
    let mut wma_values: Vec<Option<Decimal>> = Vec::with_capacity(close.len());

    // Sum of weights = n * (n + 1) / 2
    let denominator_i32 = (period * (period + 1) / 2) as u64;
    let denominator = Decimal::from_u64(denominator_i32).context("Invalid denominator")?;

    let mut window: std::collections::VecDeque<Decimal> = std::collections::VecDeque::with_capacity(period);

    for val_opt in close {
        match val_opt {
            Some(d) => {
                window.push_back(*d);
                if window.len() > period {
                    window.pop_front();
                }

                if window.len() == period {
                    let mut sum = Decimal::ZERO;
                    for (j, w_val) in window.iter().enumerate() {
                        let weight = Decimal::from_usize(j + 1).context("Invalid weight")?;
                        sum += *w_val * weight;
                    }
                    let wma = sum.checked_div(denominator).context("Division by zero in WMA")?;
                    wma_values.push(Some(wma));
                } else {
                    wma_values.push(None);
                }
            }
            None => {
                wma_values.push(None);
                window.clear();
            }
        }
    }

    Ok(wma_values)
}

/// Calculate Hull Moving Average (HMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with indicator values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::hull_moving_average;
///
/// let df = DataFrame::new(vec![Series::new("close", &[10.0, 11.0, 12.0, 13.0, 14.0])]).unwrap();
/// let result = hull_moving_average::calculate(&df, 4).unwrap();
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period < 2 {
        anyhow::bail!("Period must be at least 2");
    }

    let close_col = data.column("close").context("DataFrame must contain 'close' column")?;
    let close_f64 = close_col.cast(&DataType::Float64)?;
    let close_ca = close_f64.f64()?;

    // Extract everything to Option<Decimal> first to satisfy strictly using Decimal
    // Note: We extract to Decimal and do all intermediate/final logic with Decimal
    let close: Vec<Option<Decimal>> = close_ca.into_iter().map(|opt| {
        opt.and_then(|val| Decimal::from_f64_retain(val))
    }).collect();

    // HMA = WMA(2 * WMA(n/2) - WMA(n), sqrt(n))
    let half_period = period / 2;
    // We need integer square root. Using f64 just for the sqrt math of the parameter.
    // This is safe and not financial math.
    let sqrt_period = (period as f64).sqrt().round() as usize;

    let wma_half = calculate_wma_decimal(&close, half_period)?;
    let wma_full = calculate_wma_decimal(&close, period)?;

    let mut raw_hma_values: Vec<Option<Decimal>> = Vec::with_capacity(close.len());
    let two = Decimal::new(2, 0);

    for i in 0..close.len() {
        let half_opt = wma_half.get(i).copied().flatten();
        let full_opt = wma_full.get(i).copied().flatten();

        if let (Some(half_dec), Some(full_dec)) = (half_opt, full_opt) {
            let val = (two * half_dec) - full_dec;
            raw_hma_values.push(Some(val));
        } else {
            raw_hma_values.push(None);
        }
    }

    let hma_values = calculate_wma_decimal(&raw_hma_values, sqrt_period)?;

    // Convert back to f64 Series as Polars doesn't have native Decimal Series type
    let hma_f64: Vec<Option<f64>> = hma_values.into_iter().map(|opt| opt.map(|d| d.to_f64().unwrap_or(f64::NAN))).collect();

    Ok(Series::new("hma", hma_f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0]
        )?;

        // manual calculation for WMA, HMA
        // Period = 4, half = 2, sqrt = 2
        // Data: 10, 11, 12, 13, 14, 15, 16
        //
        // WMA(2):
        // w(10, 11) = (10*1 + 11*2)/3 = 32/3 = 10.666...
        // w(11, 12) = (11*1 + 12*2)/3 = 35/3 = 11.666...
        // w(12, 13) = (12*1 + 13*2)/3 = 38/3 = 12.666...
        // w(13, 14) = (13*1 + 14*2)/3 = 41/3 = 13.666...
        // w(14, 15) = (14*1 + 15*2)/3 = 44/3 = 14.666...
        // w(15, 16) = (15*1 + 16*2)/3 = 47/3 = 15.666...
        //
        // WMA(4):
        // w(10..13) = (10*1 + 11*2 + 12*3 + 13*4)/10 = (10 + 22 + 36 + 52)/10 = 120/10 = 12.0
        // w(11..14) = (11*1 + 12*2 + 13*3 + 14*4)/10 = (11 + 24 + 39 + 56)/10 = 130/10 = 13.0
        // w(12..15) = (12*1 + 13*2 + 14*3 + 15*4)/10 = (12 + 26 + 42 + 60)/10 = 140/10 = 14.0
        // w(13..16) = (13*1 + 14*2 + 15*3 + 16*4)/10 = (13 + 28 + 45 + 64)/10 = 150/10 = 15.0
        //
        // 2 * WMA(2) - WMA(4):
        // i=3: 2*(12.666...) - 12.0 = 25.333... - 12.0 = 13.333...
        // i=4: 2*(13.666...) - 13.0 = 27.333... - 13.0 = 14.333...
        // i=5: 2*(14.666...) - 14.0 = 29.333... - 14.0 = 15.333...
        // i=6: 2*(15.666...) - 15.0 = 31.333... - 15.0 = 16.333...
        //
        // WMA(2) of raw HMA:
        // i=4: (13.333... * 1 + 14.333... * 2) / 3 = (13.333... + 28.666...) / 3 = 42.0 / 3 = 14.0
        // i=5: (14.333... * 1 + 15.333... * 2) / 3 = (14.333... + 30.666...) / 3 = 45.0 / 3 = 15.0
        // i=6: (15.333... * 1 + 16.333... * 2) / 3 = (15.333... + 32.666...) / 3 = 48.0 / 3 = 16.0

        let result = calculate(&df, 4)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());
        assert!(out.get(3).is_none());

        let val_4 = out.get(4).unwrap();
        assert!((val_4 - 14.0).abs() < 1e-4);

        let val_5 = out.get(5).unwrap();
        assert!((val_5 - 15.0).abs() < 1e-4);

        let val_6 = out.get(6).unwrap();
        assert!((val_6 - 16.0).abs() < 1e-4);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());

        // invalid period
        let df_valid = df!("close" => &[1.0, 2.0, 3.0])?;
        let res_invalid_period = calculate(&df_valid, 1);
        assert!(res_invalid_period.is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[100.0, 102.0, 101.0, 103.0, 102.0, 105.0, 104.0, 106.0]
        )?;
        let s = calculate(&df, 4)?;
        assert_eq!(s.len(), 8);
        Ok(())
    }
}
