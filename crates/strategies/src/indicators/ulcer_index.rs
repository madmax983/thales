//! Ulcer Index (UI)
//!
//! Calculates the Ulcer Index, a technical indicator that measures downside risk
//! in terms of both the depth and duration of price declines. The index increases
//! in value as the price moves farther away from a recent high and falls as the price
//! rises to new highs.
//!
//! # References
//! - [Investopedia - Ulcer Index](https://www.investopedia.com/terms/u/ulcerindex.asp)

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate the Ulcer Index (UI)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period (standard is 14)
///
/// # Returns
/// Series with Ulcer Index values. The first `period - 1` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let ui = strategies::indicators::ulcer_index::calculate(&df, 14)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let close_vec: Vec<Option<f64>> = close.into_iter().collect();
    let max_close_vec = rolling_max_opt(&close_vec, period);

    let mut ui_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let period_dec = Decimal::from_usize(period).context("Invalid period")?;
    let hundred = Decimal::new(100, 0);

    for i in 0..close_vec.len() {
        if i < period - 1 {
            ui_values.push(None);
            continue;
        }

        let mut sum_sq_drawdown = Decimal::ZERO;
        let mut all_valid = true;

        let max_val_opt = max_close_vec[i];

        for j in (i + 1 - period)..=i {
            if let (Some(val), Some(max_val)) = (close_vec[j], max_val_opt) {
                if val.is_finite() && max_val.is_finite() {
                    let d = Decimal::from_f64_retain(val).unwrap_or(Decimal::ZERO);
                    let max_c = Decimal::from_f64_retain(max_val).unwrap_or(Decimal::ZERO);

                    if max_c.is_zero() {
                        all_valid = false;
                        break;
                    }

                    let drawdown = ((d - max_c) / max_c) * hundred;
                    sum_sq_drawdown += drawdown * drawdown;
                } else {
                    all_valid = false;
                    break;
                }
            } else {
                all_valid = false;
                break;
            }
        }

        if !all_valid {
            ui_values.push(None);
            continue;
        }

        let avg_sq_drawdown = sum_sq_drawdown / period_dec;
        let ulcer_index = decimal_sqrt(avg_sq_drawdown);

        ui_values.push(ulcer_index.and_then(|dec| dec.to_f64()));
    }

    Ok(Series::new("ulcer_index", ui_values))
}

fn rolling_max_opt(values: &[Option<f64>], window_size: usize) -> Vec<Option<f64>> {
    if window_size == 0 {
        return vec![None; values.len()];
    }
    let mut result = Vec::with_capacity(values.len());
    let mut deque: VecDeque<usize> = VecDeque::new();

    for i in 0..values.len() {
        while let Some(&front) = deque.front() {
            if front + window_size <= i {
                deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = values[i] {
            while let Some(&back) = deque.back() {
                if let Some(back_val) = values[back] {
                    if back_val <= val {
                        deque.pop_back();
                    } else {
                        break;
                    }
                } else {
                    deque.pop_back();
                }
            }
            deque.push_back(i);
        }

        if i >= window_size - 1 {
            if let Some(&front) = deque.front() {
                result.push(values[front]);
            } else {
                result.push(None);
            }
        } else {
            result.push(None);
        }
    }
    result
}

/// Computes the square root of a Decimal using Babylonian method
fn decimal_sqrt(number: Decimal) -> Option<Decimal> {
    if number < Decimal::ZERO {
        return None;
    }
    if number.is_zero() {
        return Some(Decimal::ZERO);
    }

    let mut guess = number / Decimal::TWO;
    let mut prev_guess = Decimal::ZERO;
    let tolerance = Decimal::new(1, 6); // 0.000001

    for _ in 0..50 {
        if guess.is_zero() {
            return Some(Decimal::ZERO);
        }

        let next_guess = (guess + number / guess) / Decimal::TWO;

        if (next_guess - guess).abs() <= tolerance {
            return Some(next_guess);
        }

        if next_guess == prev_guess {
            return Some(next_guess);
        }

        prev_guess = guess;
        guess = next_guess;
    }

    Some(guess)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 9.0, 8.0, 10.0]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).expect("Expected value at index 2");
        assert!((val2 - 12.909944).abs() < 1e-4);

        let val3 = out.get(3).expect("Expected value at index 3");
        assert!((val3 - 12.909944).abs() < 1e-4);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        let df_valid = df!("close" => &[10.0])?;
        let res_zero = calculate(&df_valid, 0);
        assert!(res_zero.is_err());
        assert_eq!(res_zero.unwrap_err().to_string(), "Period must be greater than 0");

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[
                100.0, 102.0, 101.0, 104.0, 107.0, 105.0, 108.0, 110.0, 109.0, 112.0
            ]
        )?;

        let result = calculate(&df, 5)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 10);
        assert!(out.get(0).is_none());
        assert!(out.get(3).is_none());
        assert!(out.get(4).is_some());

        Ok(())
    }
}
