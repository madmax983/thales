use super::{ema, macd};
use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use std::collections::VecDeque;

/// Calculate Schaff Trend Cycle (STC)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `fast_period` - MACD Fast EMA period (default 23)
/// * `slow_period` - MACD Slow EMA period (default 50)
/// * `period` - Stochastic/EMA period (default 10)
///
/// # Returns
/// Series with STC values.
pub fn calculate(
    data: &DataFrame,
    fast_period: usize,
    slow_period: usize,
    period: usize,
) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    // 1. Calculate MACD
    let (macd_line, _, _) = macd::calculate(data, fast_period, slow_period, 9)?;
    let macd_arr = macd_line.f64()?;

    let macd_dec: Vec<Option<Decimal>> = macd_arr
        .into_iter()
        .map(|v| v.and_then(Decimal::from_f64_retain))
        .collect();

    // 2. 1st Stochastic %K of MACD
    let macd_min = rolling_min(&macd_dec, period);
    let macd_max = rolling_max(&macd_dec, period);

    let mut k1_values = vec![None; data.height()];
    let hundred = Decimal::new(100, 0);

    for i in 0..data.height() {
        if let (Some(m), Some(ll), Some(hh)) = (macd_dec[i], macd_min[i], macd_max[i]) {
            let range = hh - ll;
            if range.is_zero() {
                k1_values[i] = Some(Decimal::new(50, 0));
            } else {
                k1_values[i] = Some(hundred * (m - ll) / range);
            }
        }
    }

    // 3. 1st Smoothed %D (EMA of %K1)
    let k1_f64: Vec<Option<f64>> = k1_values
        .iter()
        .map(|v| v.and_then(|d| d.to_f64()))
        .collect();
    let k1_df = DataFrame::new(vec![Series::new("close", &k1_f64)])?;

    // EMA smoothing period is period/2, but default STC uses period/2 rounded down?
    // Typical STC uses EMA of period/2. 10 / 2 = 5.
    let d1_series = ema::calculate(&k1_df, period / 2).context("Failed to calculate D1")?;
    let d1_arr = d1_series.f64()?;

    let d1_dec: Vec<Option<Decimal>> = d1_arr
        .into_iter()
        .map(|v| v.and_then(Decimal::from_f64_retain))
        .collect();

    // 4. 2nd Stochastic %K of %D1
    let d1_min = rolling_min(&d1_dec, period);
    let d1_max = rolling_max(&d1_dec, period);

    let mut k2_values = vec![None; data.height()];

    for i in 0..data.height() {
        if let (Some(d), Some(ll), Some(hh)) = (d1_dec[i], d1_min[i], d1_max[i]) {
            let range = hh - ll;
            if range.is_zero() {
                k2_values[i] = Some(Decimal::new(50, 0));
            } else {
                k2_values[i] = Some(hundred * (d - ll) / range);
            }
        }
    }

    // 5. 2nd Smoothed %D (STC) = EMA of %K2
    let k2_f64: Vec<Option<f64>> = k2_values
        .iter()
        .map(|v| v.and_then(|d| d.to_f64()))
        .collect();
    let k2_df = DataFrame::new(vec![Series::new("close", &k2_f64)])?;
    let stc_series = ema::calculate(&k2_df, period / 2).context("Failed to calculate STC")?;

    // The name of the resulting series should reflect what it is
    let mut stc_series = stc_series;
    stc_series.rename("stc");

    Ok(stc_series)
}

/// Calculate Rolling Min using Monotonic Queue (O(N))
fn rolling_min(data: &[Option<Decimal>], window: usize) -> Vec<Option<Decimal>> {
    let mut result = vec![None; data.len()];
    let mut deque: VecDeque<usize> = VecDeque::new();
    let mut none_count = 0;

    for i in 0..data.len() {
        if i >= window && data[i - window].is_none() {
            none_count -= 1;
        }

        if data[i].is_none() {
            none_count += 1;
        }

        while let Some(&front) = deque.front() {
            if front + window <= i {
                deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = data[i] {
            while let Some(&back) = deque.back() {
                if let Some(back_val) = data[back] {
                    if back_val >= val {
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

        if i >= window - 1 {
            if none_count == 0 {
                if let Some(&front) = deque.front() {
                    result[i] = data[front];
                }
            } else {
                result[i] = None;
            }
        }
    }
    result
}

/// Calculate Rolling Max using Monotonic Queue (O(N))
fn rolling_max(data: &[Option<Decimal>], window: usize) -> Vec<Option<Decimal>> {
    let mut result = vec![None; data.len()];
    let mut deque: VecDeque<usize> = VecDeque::new();
    let mut none_count = 0;

    for i in 0..data.len() {
        if i >= window && data[i - window].is_none() {
            none_count -= 1;
        }

        if data[i].is_none() {
            none_count += 1;
        }

        while let Some(&front) = deque.front() {
            if front + window <= i {
                deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = data[i] {
            while let Some(&back) = deque.back() {
                if let Some(back_val) = data[back] {
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

        if i >= window - 1 {
            if none_count == 0 {
                if let Some(&front) = deque.front() {
                    result[i] = data[front];
                }
            } else {
                result[i] = None;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_stc_edge_cases() -> Result<()> {
        let df_empty = DataFrame::empty();
        let res = calculate(&df_empty, 23, 50, 10);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5, 10, 3)?;
        assert_eq!(res_short.len(), 2);

        let df_flat = df!("close" => &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0])?;
        let res_flat = calculate(&df_flat, 2, 4, 2)?;
        assert_eq!(res_flat.len(), 6);
        Ok(())
    }

    #[test]
    fn test_stc_calculation() -> Result<()> {
        let mut closes = Vec::new();
        for i in 0..100 {
            closes.push(100.0 + (i as f64 * 0.1).sin() * 10.0);
        }
        let df = df!("close" => closes)?;

        let stc = calculate(&df, 23, 50, 10)?;
        let stc_arr = stc.f64()?;

        assert_eq!(stc_arr.len(), 100);

        if let Some(val) = stc_arr.get(99) {
            assert!((0.0..=100.0).contains(&val));
        }

        Ok(())
    }
}
