//! Stochastic Oscillator Indicator
//!
//! The Stochastic Oscillator is a momentum indicator comparing a particular closing price
//! of a security to a range of its prices over a certain period of time.
//!
//! Formula:
//! %K = 100 * (Close - Lowest Low) / (Highest High - Lowest Low)
//! %D = SMA(%K, d_period)

use anyhow::Result;
use polars::prelude::*;

/// Calculate Stochastic Oscillator (%K and %D)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns
/// * `k_period` - Lookback period for %K (e.g., 14)
/// * `k_smoothing` - Smoothing period for %K (e.g., 3). If 1, it's Fast Stochastic. If 3, it's Slow Stochastic.
/// * `d_period` - Smoothing period for %D (e.g., 3)
///
/// # Returns
/// Tuple of (Series %K, Series %D). First few values will be null.
pub fn calculate(
    data: &DataFrame,
    k_period: usize,
    k_smoothing: usize,
    d_period: usize,
) -> Result<(Series, Series)> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if k_period == 0 || k_smoothing == 0 || d_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

    let close = data.column("close")?;
    let high = data.column("high")?;
    let low = data.column("low")?;

    let lowest_low = calculate_rolling_min(low, k_period)?;
    let highest_high = calculate_rolling_max(high, k_period)?;

    let close_f64 = close.f64()?;
    let low_f64 = lowest_low.f64()?;
    let high_f64 = highest_high.f64()?;

    let mut raw_k_values: Vec<Option<f64>> = Vec::with_capacity(data.height());

    for i in 0..data.height() {
        let c = close_f64.get(i);
        let l = low_f64.get(i);
        let h = high_f64.get(i);

        if let (Some(c_val), Some(l_val), Some(h_val)) = (c, l, h) {
            if (h_val - l_val).abs() < f64::EPSILON {
                 // Avoid division by zero.
                 // If High == Low, price is flat. %K is technically 100 or 50 or 0.
                 // Let's say 50.
                 raw_k_values.push(Some(50.0));
            } else {
                let k = 100.0 * (c_val - l_val) / (h_val - l_val);
                raw_k_values.push(Some(k));
            }
        } else {
            raw_k_values.push(None);
        }
    }

    let raw_k_series = Series::new("raw_k", raw_k_values);

    // Smooth %K
    let k_series = if k_smoothing > 1 {
        // Simple Moving Average of Raw %K
        calculate_sma(&raw_k_series, k_smoothing)?
    } else {
        raw_k_series
    };

    // Calculate %D (SMA of %K)
    let d_series = calculate_sma(&k_series, d_period)?;

    Ok((k_series, d_series))
}

fn calculate_rolling_min(series: &Series, window: usize) -> Result<Series> {
    let arr = series.f64()?;
    let mut result: Vec<Option<f64>> = vec![None; arr.len()];
    let mut deque: std::collections::VecDeque<usize> = std::collections::VecDeque::new();

    for i in 0..arr.len() {
        // Remove indices out of window
        if let Some(&front) = deque.front() {
            if front + window <= i {
                deque.pop_front();
            }
        }

        if let Some(val) = arr.get(i) {
            // Maintain monotonic increasing order for Min
            while let Some(&back) = deque.back() {
                if let Some(back_val) = arr.get(back) {
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

        // Window is valid from index `window - 1`
        if i >= window - 1 {
             if let Some(&front) = deque.front() {
                 result[i] = arr.get(front);
             }
        }
    }

    Ok(Series::new(series.name(), result))
}

fn calculate_rolling_max(series: &Series, window: usize) -> Result<Series> {
    let arr = series.f64()?;
    let mut result: Vec<Option<f64>> = vec![None; arr.len()];
    let mut deque: std::collections::VecDeque<usize> = std::collections::VecDeque::new();

    for i in 0..arr.len() {
        // Remove indices out of window
        if let Some(&front) = deque.front() {
            if front + window <= i {
                deque.pop_front();
            }
        }

        if let Some(val) = arr.get(i) {
            // Maintain monotonic decreasing order for Max
            while let Some(&back) = deque.back() {
                if let Some(back_val) = arr.get(back) {
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
             if let Some(&front) = deque.front() {
                 result[i] = arr.get(front);
             }
        }
    }

    Ok(Series::new(series.name(), result))
}

fn calculate_sma(series: &Series, window: usize) -> Result<Series> {
    let arr = series.f64()?;
    let mut result: Vec<Option<f64>> = vec![None; arr.len()];

    let mut sum = 0.0;
    let mut valid_count = 0;
    let mut window_vals = std::collections::VecDeque::new();

    for i in 0..arr.len() {
        let val_opt = arr.get(i);
        window_vals.push_back(val_opt);

        if let Some(v) = val_opt {
            sum += v;
            valid_count += 1;
        }

        if window_vals.len() > window {
            let popped = window_vals.pop_front().unwrap();
            if let Some(v) = popped {
                sum -= v;
                valid_count -= 1;
            }
        }

        if window_vals.len() == window {
            if valid_count == window {
                result[i] = Some(sum / window as f64);
            } else {
                result[i] = None;
            }
        }
    }

    Ok(Series::new(series.name(), result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stochastic_calculation() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 12.0, 11.0, 13.0, 14.0],
            "low" => &[5.0, 6.0, 7.0, 8.0, 9.0],
            "close" => &[8.0, 10.0, 9.0, 12.0, 13.0]
        )?;

        let (k, d) = calculate(&df, 3, 1, 2)?;

        let k_arr = k.f64()?;
        let d_arr = d.f64()?;

        assert!(k_arr.get(0).is_none());
        assert!(k_arr.get(1).is_none());

        // Check K
        let k2 = k_arr.get(2).unwrap();
        assert!((k2 - 57.14).abs() < 0.01, "K[2] expected 57.14, got {}", k2);

        let k3 = k_arr.get(3).unwrap();
        assert!((k3 - 85.71).abs() < 0.01, "K[3] expected 85.71, got {}", k3);

        // Check D
        assert!(d_arr.get(2).is_none()); // Need 2 values of K (at 2 and 3) to get D at 3

        let d3 = d_arr.get(3).unwrap();
        assert!((d3 - 71.425).abs() < 0.01, "D[3] expected 71.425, got {}", d3);

        Ok(())
    }
}
