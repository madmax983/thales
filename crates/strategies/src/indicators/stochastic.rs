//! Stochastic Oscillator Indicator
//!
//! The Stochastic Oscillator is a momentum indicator comparing a particular closing price
//! of a security to a range of its prices over a certain period of time.
//!
//! Formula:
//! %K = 100 * (Close - Lowest Low) / (Highest High - Lowest Low)
//! %D = SMA(%K, d_period)

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use std::collections::VecDeque;

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
/// Series names: "stochastic_k", "stochastic_d"
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

    let high_series = data
        .column("high")
        .context("Missing 'high' column")?
        .f64()?;
    let low_series = data.column("low").context("Missing 'low' column")?.f64()?;
    let close_series = data
        .column("close")
        .context("Missing 'close' column")?
        .f64()?;

    // Convert to Decimal for precision, treating NaN/Inf as None
    let highs: Vec<Option<Decimal>> = high_series
        .into_iter()
        .map(|v| v.and_then(|f| Decimal::from_f64_retain(f)))
        .collect();
    let lows: Vec<Option<Decimal>> = low_series
        .into_iter()
        .map(|v| v.and_then(|f| Decimal::from_f64_retain(f)))
        .collect();
    let closes: Vec<Option<Decimal>> = close_series
        .into_iter()
        .map(|v| v.and_then(|f| Decimal::from_f64_retain(f)))
        .collect();

    // 1. Calculate Lowest Low and Highest High over k_period
    let lowest_low = rolling_min(&lows, k_period);
    let highest_high = rolling_max(&highs, k_period);

    // 2. Calculate Raw %K
    // %K = 100 * (Close - Lowest Low) / (Highest High - Lowest Low)
    let mut raw_k = vec![None; data.height()];
    let hundred = Decimal::new(100, 0);

    for i in 0..data.height() {
        if let (Some(c), Some(ll), Some(hh)) = (closes[i], lowest_low[i], highest_high[i]) {
            let range = hh - ll;
            if range.is_zero() {
                // If High == Low, price is flat. Undefined mathematically.
                // Convention: 50 (neutral).
                raw_k[i] = Some(Decimal::new(50, 0));
            } else {
                let k = hundred * (c - ll) / range;
                raw_k[i] = Some(k);
            }
        }
    }

    // 3. Smooth %K (if k_smoothing > 1)
    let k_values = if k_smoothing > 1 {
        calculate_sma(&raw_k, k_smoothing)
    } else {
        raw_k
    };

    // 4. Calculate %D (SMA of %K)
    let d_values = calculate_sma(&k_values, d_period);

    // Convert back to f64 Series
    let k_f64: Vec<Option<f64>> = k_values
        .into_iter()
        .map(|d| d.map(|v| v.to_f64().unwrap_or(0.0)))
        .collect();
    let d_f64: Vec<Option<f64>> = d_values
        .into_iter()
        .map(|d| d.map(|v| v.to_f64().unwrap_or(0.0)))
        .collect();

    let k_series = Series::new("stochastic_k", k_f64);
    let d_series = Series::new("stochastic_d", d_f64);

    Ok((k_series, d_series))
}

/// Calculate Rolling Min using Monotonic Queue (O(N))
fn rolling_min(data: &[Option<Decimal>], window: usize) -> Vec<Option<Decimal>> {
    let mut result = vec![None; data.len()];
    let mut deque: VecDeque<usize> = VecDeque::new();
    let mut none_count = 0;

    for i in 0..data.len() {
        // Leaving window
        if i >= window {
            if data[i - window].is_none() {
                none_count -= 1;
            }
        }

        // Entering window
        if data[i].is_none() {
            none_count += 1;
        }

        // Remove indices out of window
        while let Some(&front) = deque.front() {
            if front + window <= i {
                deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = data[i] {
            // Maintain increasing order
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

        // Result
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
        // Leaving window
        if i >= window {
            if data[i - window].is_none() {
                none_count -= 1;
            }
        }

        // Entering window
        if data[i].is_none() {
            none_count += 1;
        }

        // Remove indices out of window
        while let Some(&front) = deque.front() {
            if front + window <= i {
                deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = data[i] {
            // Maintain decreasing order
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

/// Calculate Simple Moving Average (SMA)
fn calculate_sma(data: &[Option<Decimal>], window: usize) -> Vec<Option<Decimal>> {
    let mut result = vec![None; data.len()];
    let mut sum = Decimal::ZERO;
    let mut count = 0;
    let mut queue: VecDeque<Option<Decimal>> = VecDeque::new();

    for i in 0..data.len() {
        let val_opt = data[i];
        queue.push_back(val_opt);

        if let Some(val) = val_opt {
            sum += val;
            count += 1;
        }

        if queue.len() > window {
            let popped = queue.pop_front().unwrap(); // Safe
            if let Some(val) = popped {
                sum -= val;
                count -= 1;
            }
        }

        if queue.len() == window {
            if count == window {
                // All values valid
                result[i] = Some(sum / Decimal::from_usize(window).unwrap());
            } else {
                // Some values were None
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
    fn test_known_values() -> Result<()> {
        // Simple case:
        // k_period = 2, k_smoothing = 1, d_period = 2

        let df = df!(
            "high" =>  &[10.0, 10.0, 10.0, 12.0],
            "low" =>   &[ 0.0,  0.0,  0.0,  2.0],
            "close" => &[ 5.0, 10.0,  0.0,  7.0]
        )?;

        let (k, d) = calculate(&df, 2, 1, 2)?;
        let k_vals = k.f64()?;
        let d_vals = d.f64()?;

        let val_k1 = k_vals.get(1);
        if let Some(v) = val_k1 {
            assert!((v - 100.0).abs() < 0.001, "K[1] expected 100.0, got {}", v);
        } else {
            assert!(val_k1.is_some(), "K[1] should be calculated");
        }

        let val_k2 = k_vals.get(2).unwrap();
        assert!(
            (val_k2 - 0.0).abs() < 0.001,
            "K[2] expected 0.0, got {}",
            val_k2
        );

        let val_k3 = k_vals.get(3).unwrap();
        assert!(
            (val_k3 - 58.333).abs() < 0.001,
            "K[3] expected 58.333, got {}",
            val_k3
        );

        let val_d2 = d_vals.get(2).unwrap();
        assert!(
            (val_d2 - 50.0).abs() < 0.001,
            "D[2] expected 50.0, got {}",
            val_d2
        );

        let val_d3 = d_vals.get(3).unwrap();
        assert!(
            (val_d3 - 29.166).abs() < 0.001,
            "D[3] expected 29.166, got {}",
            val_d3
        );

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14, 3, 3);
        assert!(res_empty.is_err());

        // Single point (less than period)
        let df_single = df!(
            "high" => &[10.0],
            "low" => &[10.0],
            "close" => &[10.0]
        )?;
        let (k, _d) = calculate(&df_single, 14, 3, 3)?;
        let k_vals = k.f64()?;
        assert_eq!(k_vals.len(), 1);
        assert!(k_vals.get(0).is_none());

        // Flat price (High == Low) -> Division by Zero check
        let df_flat = df!(
            "high" =>  &[10.0, 10.0, 10.0],
            "low" =>   &[10.0, 10.0, 10.0],
            "close" => &[10.0, 10.0, 10.0]
        )?;

        let (k_flat, _) = calculate(&df_flat, 2, 1, 2)?;
        // Should calculate 50.0 (neutral)
        let k_flat_vals = k_flat.f64()?;
        assert_eq!(k_flat_vals.get(1), Some(50.0));

        // NaN Handling
        let df_nan = df!(
            "high" =>  &[10.0, 10.0, f64::NAN, 12.0],
            "low" =>   &[ 0.0,  0.0,  0.0,  2.0],
            "close" => &[ 5.0, 10.0,  0.0,  7.0]
        )?;
        let (k_nan, _) = calculate(&df_nan, 2, 1, 2)?;
        let k_nan_vals = k_nan.f64()?;
        // Index 2 has NaN High. Window [1, 2]. Result should be None (Strict).
        assert!(
            k_nan_vals.get(2).is_none(),
            "Index 2 should be None due to NaN in window"
        );

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let highs: Vec<f64> = values.iter().map(|v| v + 1.0).collect();
        let lows: Vec<f64> = values.iter().map(|v| v - 1.0).collect();
        let closes = values;

        let df = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let (k, d) = calculate(&df, 14, 3, 3)?;
        assert_eq!(k.len(), 100);
        assert_eq!(d.len(), 100);

        let k_arr = k.f64()?;
        if let Some(val) = k_arr.get(20) {
            assert!(
                val >= 0.0 && val <= 100.0,
                "K value {} out of range [0, 100]",
                val
            );
        }

        Ok(())
    }
}
