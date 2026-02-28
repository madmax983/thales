//! Donchian Channels - A volatility indicator using the highest high and lowest low.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Donchian Channels
///
/// # Arguments
/// * `data` - DataFrame with "high" and "low" columns.
/// * `period` - Lookback period.
///
/// # Returns
/// Tuple of (Lower Band, Middle Band, Upper Band) Series.
pub fn calculate(data: &DataFrame, period: usize) -> Result<(Series, Series, Series)> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Get "high" and "low" columns
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

    // Convert to Vec<f64> for manual calculation
    // Handling nulls: treat as breaks? Or skip?
    // For simplicity and robustness matching strategy implementation:
    // We treat nulls as gaps.

    // Note: To be strictly correct with Polars null handling, we should probably iterate options.
    // The strategy implementation uses `into_no_null_iter()` which might be dangerous if there are nulls.
    // We will use iteration with Option.

    let highs_vec: Vec<Option<f64>> = high.into_iter().collect();
    let lows_vec: Vec<Option<f64>> = low.into_iter().collect();

    let upper_vals = rolling_max_opt(&highs_vec, period);
    let lower_vals = rolling_min_opt(&lows_vec, period);

    // Calculate Middle Band = (Upper + Lower) / 2
    let mut middle_vals = Vec::with_capacity(high.len());
    let two = Decimal::from(2);

    for i in 0..high.len() {
        match (upper_vals[i], lower_vals[i]) {
            (Some(u), Some(l)) => {
                if let (Some(u_dec), Some(l_dec)) =
                    (Decimal::from_f64_retain(u), Decimal::from_f64_retain(l))
                {
                    let m = (u_dec + l_dec) / two;
                    middle_vals.push(m.to_f64());
                } else {
                    middle_vals.push(None);
                }
            }
            _ => middle_vals.push(None),
        }
    }

    let s_lower = Series::new("donchian_lower", lower_vals);
    let s_middle = Series::new("donchian_middle", middle_vals);
    let s_upper = Series::new("donchian_upper", upper_vals);

    Ok((s_lower, s_middle, s_upper))
}

// Helper functions for rolling calculations using Monotonic Queue (O(N))
// Adapted to handle Option<f64>
fn rolling_max_opt(values: &[Option<f64>], window_size: usize) -> Vec<Option<f64>> {
    if window_size == 0 {
        return vec![None; values.len()];
    }
    let mut result = Vec::with_capacity(values.len());
    let mut deque: VecDeque<usize> = VecDeque::new();

    for i in 0..values.len() {
        // Remove indices out of window
        while let Some(&front) = deque.front() {
            if front + window_size <= i {
                deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = values[i] {
            // Maintain decreasing order for Max
            while let Some(&back) = deque.back() {
                if let Some(back_val) = values[back] {
                    if back_val <= val {
                        deque.pop_back();
                    } else {
                        break;
                    }
                } else {
                    // Should not happen if we only push indices with Some
                    deque.pop_back();
                }
            }
            deque.push_back(i);
        }

        // Result for this window
        // We need to check if the window is valid (e.g. at least 1 value? or full window?)
        // Standard rolling usually requires full window or min_periods.
        // Let's mimic Polars default: min_periods=window_size.
        if i >= window_size - 1 {
            // Check if we have a valid max in the window
            if let Some(&front) = deque.front() {
                result.push(values[front]);
            } else {
                // All None in window
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
        // Remove indices out of window
        while let Some(&front) = deque.front() {
            if front + window_size <= i {
                deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = values[i] {
            // Maintain increasing order for Min
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
    fn test_known_values() -> Result<()> {
        // Highs: 10, 15, 12, 18, 20
        // Lows:   5,  8,  6, 10, 15
        // Period: 3

        // Index 0: NA
        // Index 1: NA
        // Index 2: Highs [10, 15, 12] -> Max 15. Lows [5, 8, 6] -> Min 5. Mid (15+5)/2 = 10.
        // Index 3: Highs [15, 12, 18] -> Max 18. Lows [8, 6, 10] -> Min 6. Mid (18+6)/2 = 12.
        // Index 4: Highs [12, 18, 20] -> Max 20. Lows [6, 10, 15] -> Min 6. Mid (20+6)/2 = 13.

        let df = df!(
            "high" => &[10.0, 15.0, 12.0, 18.0, 20.0],
            "low" =>  &[ 5.0,  8.0,  6.0, 10.0, 15.0]
        )?;

        let (lower, middle, upper) = calculate(&df, 3)?;

        let l = lower.f64()?;
        let m = middle.f64()?;
        let u = upper.f64()?;

        assert!(u.get(0).is_none());
        assert!(u.get(1).is_none());

        assert_eq!(u.get(2), Some(15.0));
        assert_eq!(l.get(2), Some(5.0));
        assert_eq!(m.get(2), Some(10.0));

        assert_eq!(u.get(3), Some(18.0));
        assert_eq!(l.get(3), Some(6.0));
        assert_eq!(m.get(3), Some(12.0));

        assert_eq!(u.get(4), Some(20.0));
        assert_eq!(l.get(4), Some(6.0));
        assert_eq!(m.get(4), Some(13.0));

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!(
            "high" => &[10.0, 11.0],
            "low" => &[5.0, 6.0]
        )?;
        let (l, m, u) = calculate(&df_short, 5)?;

        assert_eq!(u.len(), 2);
        assert!(u.f64()?.get(0).is_none());
        assert!(u.f64()?.get(1).is_none());
        assert!(l.f64()?.get(1).is_none());
        assert!(m.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64)).collect();
        let df = df!(
            "high" => values.clone(),
            "low" => values.clone()
        )?;

        let (l, m, u) = calculate(&df, 14)?;
        assert_eq!(l.len(), 100);
        assert_eq!(m.len(), 100);
        assert_eq!(u.len(), 100);

        // Check first few are None
        for i in 0..13 {
            assert!(u.f64()?.get(i).is_none());
        }
        // Check 14th is valid
        assert!(u.f64()?.get(13).is_some());

        Ok(())
    }
}
