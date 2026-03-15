use anyhow::Result;
use polars::prelude::*;

use std::collections::VecDeque;

/// Calculates the Williams %R indicator.
///
/// %R = (Highest High - Close) / (Highest High - Lowest Low) * -100
///
/// # Arguments
///
/// * `data` - DataFrame containing "high", "low", and "close" columns.
/// * `period` - The lookback window for highest high and lowest low (e.g., 14).
///
/// # Returns
///
/// A `Series` containing the %R values. The first `period - 1` values will be null.
///
/// # Examples
///
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::williams_r::calculate;
///
/// let df = df!(
///     "high" => &[12.0, 11.0, 10.0, 11.0, 12.0],
///     "low" => &[10.0, 9.0, 8.0, 9.0, 10.0],
///     "close" => &[11.0, 10.0, 9.0, 10.0, 11.0]
/// ).unwrap();
///
/// let result = calculate(&df, 3).unwrap();
/// assert_eq!(result.name(), "williams_r");
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    let high = data.column("high")?.f64()?;
    let low = data.column("low")?.f64()?;
    let close = data.column("close")?.f64()?;

    let highs_vec: Vec<Option<f64>> = high.into_iter().collect();
    let lows_vec: Vec<Option<f64>> = low.into_iter().collect();

    let highest_high = rolling_max_opt(&highs_vec, period);
    let lowest_low = rolling_min_opt(&lows_vec, period);

    let mut wr_vals = Vec::with_capacity(data.height());

    for i in 0..data.height() {
        match (highest_high[i], lowest_low[i], close.get(i)) {
            (Some(hh), Some(ll), Some(c)) => {
                let diff_high_low = hh - ll;
                if diff_high_low == 0.0 {
                    // Avoid division by zero
                    wr_vals.push(Some(-50.0));
                } else {
                    let diff_high_close = hh - c;
                    let wr = (diff_high_close / diff_high_low) * -100.0;
                    wr_vals.push(Some(wr));
                }
            }
            _ => wr_vals.push(None),
        }
    }

    Ok(Series::new("williams_r", wr_vals))
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

fn rolling_min_opt(values: &[Option<f64>], window_size: usize) -> Vec<Option<f64>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_williams_r() -> Result<()> {
        let df = df!(
            "high"  => &[12.0, 11.0, 10.0, 11.0, 12.0],
            "low"   => &[10.0, 9.0, 8.0, 9.0, 10.0],
            "close" => &[11.0, 10.0, 9.0, 10.0, 11.0]
        )?;

        // Period 3
        let wr = calculate(&df, 3)?;
        let wr_arr = wr.f64()?;

        // i=0: null
        // i=1: null
        // i=2: high=[12, 11, 10], hh=12. low=[10, 9, 8], ll=8. close=9.
        // %R = (12 - 9) / (12 - 8) * -100 = 3 / 4 * -100 = -75.0
        assert_eq!(wr_arr.get(2), Some(-75.0));

        // i=3: high=[11, 10, 11], hh=11. low=[9, 8, 9], ll=8. close=10.
        // %R = (11 - 10) / (11 - 8) * -100 = 1 / 3 * -100 = -33.333...
        assert!((wr_arr.get(3).unwrap() - -33.333).abs() < 0.01);

        // i=4: high=[10, 11, 12], hh=12. low=[8, 9, 10], ll=8. close=11.
        // %R = (12 - 11) / (12 - 8) * -100 = 1 / 4 * -100 = -25.0
        assert_eq!(wr_arr.get(4), Some(-25.0));

        Ok(())
    }
}
