//! Chandelier Exit
//!
//! A volatility-based indicator that uses the Average True Range (ATR) to trail a stop loss.
//! It is commonly used as a trend-following system to keep traders in a trend until a defined trend reversal occurs.
//!
//! Long Exit = Highest High(period) - (ATR(period) * multiplier)
//! Short Exit = Lowest Low(period) + (ATR(period) * multiplier)

use crate::indicators::atr;
use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Chandelier Exit
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns
/// * `period` - Lookback period for Highest High, Lowest Low, and ATR (standard is 22)
/// * `multiplier` - Multiplier for ATR (standard is 3.0)
///
/// # Returns
/// Tuple of (Long Exit, Short Exit) Series. The first `period - 1` values will be null.
pub fn calculate(data: &DataFrame, period: usize, multiplier: f64) -> Result<(Series, Series)> {
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

    // Calculate ATR
    let atr_series = atr::calculate(data, period)?;
    let atr = atr_series.f64()?;

    let len = high.len();
    let mut long_exit_vals: Vec<Option<f64>> = vec![None; len];
    let mut short_exit_vals: Vec<Option<f64>> = vec![None; len];

    if len < period {
        return Ok((
            Series::new("chandelier_long", long_exit_vals),
            Series::new("chandelier_short", short_exit_vals),
        ));
    }

    let mult_dec = Decimal::from_f64_retain(multiplier).unwrap_or(Decimal::ZERO);

    // O(N) rolling max for highs
    let mut max_deque: VecDeque<usize> = VecDeque::new();
    // O(N) rolling min for lows
    let mut min_deque: VecDeque<usize> = VecDeque::new();

    for i in 0..len {
        let curr_high_opt = high.get(i);
        let curr_low_opt = low.get(i);
        let atr_val_opt = atr.get(i);

        // Remove old elements from max_deque
        while let Some(&front_idx) = max_deque.front() {
            if i >= period && front_idx <= i - period {
                max_deque.pop_front();
            } else {
                break;
            }
        }

        // Remove old elements from min_deque
        while let Some(&front_idx) = min_deque.front() {
            if i >= period && front_idx <= i - period {
                min_deque.pop_front();
            } else {
                break;
            }
        }

        // Update rolling max
        if let Some(curr_h) = curr_high_opt {
            while let Some(&back_idx) = max_deque.back() {
                if let Some(back_val) = high.get(back_idx) {
                    if back_val <= curr_h {
                        max_deque.pop_back();
                    } else {
                        break;
                    }
                } else {
                    max_deque.pop_back();
                }
            }
            max_deque.push_back(i);
        }

        // Update rolling min
        if let Some(curr_l) = curr_low_opt {
            while let Some(&back_idx) = min_deque.back() {
                if let Some(back_val) = low.get(back_idx) {
                    if back_val >= curr_l {
                        min_deque.pop_back();
                    } else {
                        break;
                    }
                } else {
                    min_deque.pop_back();
                }
            }
            min_deque.push_back(i);
        }

        if i >= period - 1 {
            if let (Some(&max_idx), Some(&min_idx), Some(atr_val)) =
                (max_deque.front(), min_deque.front(), atr_val_opt)
            {
                if let (Some(max_h), Some(min_l)) = (high.get(max_idx), low.get(min_idx)) {
                    let highest_high = Decimal::from_f64_retain(max_h).unwrap_or(Decimal::ZERO);
                    let lowest_low = Decimal::from_f64_retain(min_l).unwrap_or(Decimal::ZERO);
                    let atr_dec = Decimal::from_f64_retain(atr_val).unwrap_or(Decimal::ZERO);

                    let long_exit = highest_high - (atr_dec * mult_dec);
                    let short_exit = lowest_low + (atr_dec * mult_dec);

                    long_exit_vals[i] = long_exit.to_f64();
                    short_exit_vals[i] = short_exit.to_f64();
                }
            }
        }
    }

    Ok((
        Series::new("chandelier_long", long_exit_vals),
        Series::new("chandelier_short", short_exit_vals),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_chandelier_exit_calculation() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 12.0, 15.0, 14.0, 13.0, 12.0],
            "low" => &[8.0, 9.0, 11.0, 10.0, 9.0, 8.0],
            "close" => &[9.0, 11.0, 14.0, 12.0, 10.0, 9.0]
        )?;

        let period = 3;
        let multiplier = 2.0;
        let (long_exit, short_exit) = calculate(&df, period, multiplier)?;

        let long = long_exit.f64()?;
        let short = short_exit.f64()?;

        assert_eq!(long.len(), 6);
        assert_eq!(short.len(), 6);

        // period is 3. ATR period is 3.
        assert!(long.get(0).is_none());
        assert!(long.get(1).is_none());

        // For index 2:
        // highs: [10, 12, 15], max_high = 15
        // lows: [8, 9, 11], min_low = 8
        // atr is positive, long_exit = 15 - (atr * 2)
        let le2 = long.get(2).unwrap();
        let se2 = short.get(2).unwrap();
        assert!(le2 < 15.0);
        assert!(se2 > 8.0);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14, 2.0);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!(
            "close" => &[100.0, 101.0],
            "high" => &[101.0, 102.0],
            "low" => &[99.0, 100.0]
        )?;
        let res_short = calculate(&df_short, 5, 2.0)?;
        let long = res_short.0.f64()?;
        assert_eq!(long.len(), 2);
        assert!(long.get(0).is_none());
        assert!(long.get(1).is_none());

        // Zero period
        let df_normal = df!(
            "close" => &[100.0, 101.0],
            "high" => &[101.0, 102.0],
            "low" => &[99.0, 100.0]
        )?;
        let res_zero = calculate(&df_normal, 0, 2.0);
        assert!(res_zero.is_err());

        // Missing columns
        let df_missing = df!("close" => &[100.0])?;
        let res_missing = calculate(&df_missing, 14, 2.0);
        assert!(res_missing.is_err());

        Ok(())
    }
}
