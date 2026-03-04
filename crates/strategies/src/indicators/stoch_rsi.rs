use anyhow::Result;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Stochastic RSI (StochRSI)
///
/// Applies the Stochastic Oscillator formula to the Relative Strength Index (RSI).
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `rsi_period` - Lookback period for RSI
/// * `stoch_period` - Lookback period for StochRSI
/// * `k_period` - Smoothing period for %K
/// * `d_period` - Smoothing period for %D
///
/// # Returns
/// A tuple of two Series `(percent_k, percent_d)` representing StochRSI %K and %D.
pub fn calculate(
    data: &DataFrame,
    rsi_period: usize,
    stoch_period: usize,
    k_period: usize,
    d_period: usize,
) -> Result<(Series, Series)> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if rsi_period == 0 || stoch_period == 0 || k_period == 0 || d_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

    // 1. Calculate RSI
    let rsi_series = crate::indicators::rsi::calculate(data, rsi_period)?;
    let rsi_arr = rsi_series.f64()?;
    let len = rsi_arr.len();

    let mut stoch_rsi_values: Vec<Option<f64>> = vec![None; len];

    // O(N) Deque for Rolling Min and Max of RSI
    let mut min_deque: VecDeque<usize> = VecDeque::new();
    let mut max_deque: VecDeque<usize> = VecDeque::new();

    for (i, stoch_rsi_val) in stoch_rsi_values.iter_mut().enumerate().take(len) {
        let val_opt = rsi_arr.get(i);

        // Remove elements not within the window
        while let Some(&idx) = min_deque.front() {
            if idx + stoch_period <= i {
                min_deque.pop_front();
            } else {
                break;
            }
        }
        while let Some(&idx) = max_deque.front() {
            if idx + stoch_period <= i {
                max_deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = val_opt {
            // Maintain min deque
            while let Some(&idx) = min_deque.back() {
                if let Some(back_val) = rsi_arr.get(idx) {
                    if back_val >= val {
                        min_deque.pop_back();
                    } else {
                        break;
                    }
                } else {
                    min_deque.pop_back();
                }
            }
            min_deque.push_back(i);

            // Maintain max deque
            while let Some(&idx) = max_deque.back() {
                if let Some(back_val) = rsi_arr.get(idx) {
                    if back_val <= val {
                        max_deque.pop_back();
                    } else {
                        break;
                    }
                } else {
                    max_deque.pop_back();
                }
            }
            max_deque.push_back(i);

            // Calculate StochRSI if we have enough data (at least stoch_period valid RSI values)
            // Wait, rsi_arr has `rsi_period` leading Nones.
            // The first valid RSI is at index `rsi_period`.
            // So the first valid StochRSI is at `rsi_period + stoch_period - 1`... wait.
            // Actually, if we just check if `min_deque` and `max_deque` have elements from the valid range,
            // we can calculate. But we need a full `stoch_period` of VALID RSI values.
            // A simple way is to check if `i >= rsi_period + stoch_period - 1`.
            if i >= rsi_period + stoch_period - 1 {
                let min_idx = *min_deque.front().unwrap();
                let max_idx = *max_deque.front().unwrap();

                let min_val = rsi_arr.get(min_idx).unwrap();
                let max_val = rsi_arr.get(max_idx).unwrap();

                let stoch_rsi = if max_val > min_val {
                    ((val - min_val) / (max_val - min_val)) * 100.0
                } else {
                    // If max == min, StochRSI is undefined or 0.
                    // Usually it means RSI has been flat, we'll set it to 0 or 50. Let's use 0.0.
                    0.0
                };
                *stoch_rsi_val = Some(stoch_rsi);
            }
        } else {
            // If val is None, we clear deques?
            // Since RSI returns None for first `rsi_period` elements, we just ignore them.
            // But if there's a None later, we might want to reset.
            min_deque.clear();
            max_deque.clear();
        }
    }

    // 2. Smooth StochRSI to get %K (SMA over k_period)
    let mut k_values: Vec<Option<f64>> = vec![None; len];
    let mut k_sum = Decimal::ZERO;
    let mut k_count = 0;
    let mut k_start_idx = 0;

    for (i, k_val) in k_values.iter_mut().enumerate().take(len) {
        if let Some(val) = stoch_rsi_values[i] {
            if let Some(d) = Decimal::from_f64_retain(val) {
                k_sum += d;
                k_count += 1;

                if k_count > k_period {
                    if let Some(out_val) = stoch_rsi_values[k_start_idx] {
                        if let Some(out_d) = Decimal::from_f64_retain(out_val) {
                            k_sum -= out_d;
                        }
                    }
                    k_count -= 1;
                    k_start_idx += 1;
                }

                if k_count == k_period {
                    let k_dec = Decimal::from_usize(k_period).unwrap();
                    let avg = k_sum / k_dec;
                    *k_val = Some(avg.to_f64().unwrap_or(0.0));
                }
            }
        } else {
            k_sum = Decimal::ZERO;
            k_count = 0;
            k_start_idx = i + 1;
        }
    }

    // 3. Smooth %K to get %D (SMA over d_period)
    let mut d_values: Vec<Option<f64>> = vec![None; len];
    let mut d_sum = Decimal::ZERO;
    let mut d_count = 0;
    let mut d_start_idx = 0;

    for (i, d_val) in d_values.iter_mut().enumerate().take(len) {
        if let Some(val) = k_values[i] {
            if let Some(d) = Decimal::from_f64_retain(val) {
                d_sum += d;
                d_count += 1;

                if d_count > d_period {
                    if let Some(out_val) = k_values[d_start_idx] {
                        if let Some(out_d) = Decimal::from_f64_retain(out_val) {
                            d_sum -= out_d;
                        }
                    }
                    d_count -= 1;
                    d_start_idx += 1;
                }

                if d_count == d_period {
                    let d_dec = Decimal::from_usize(d_period).unwrap();
                    let avg = d_sum / d_dec;
                    *d_val = Some(avg.to_f64().unwrap_or(0.0));
                }
            }
        } else {
            d_sum = Decimal::ZERO;
            d_count = 0;
            d_start_idx = i + 1;
        }
    }

    let k_series = Series::new("stoch_rsi_k", k_values);
    let d_series = Series::new("stoch_rsi_d", d_values);

    Ok((k_series, d_series))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stoch_rsi_calculation() -> Result<()> {
        // Let's create a DataFrame with enough data points.
        // For rsi_period=3, stoch_period=3, k_period=3, d_period=3
        // We need at least 3 + 3 + 3 + 3 = 12 points to see a valid %D.
        let values: Vec<f64> = vec![
            10.0, 11.0, 12.0, 11.0, 10.0, 9.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0,
        ];
        let df = df!("close" => values)?;

        let (k_series, d_series) = calculate(&df, 3, 3, 3, 3)?;
        let k_arr = k_series.f64()?;
        let d_arr = d_series.f64()?;

        assert_eq!(k_arr.len(), 14);
        assert_eq!(d_arr.len(), 14);

        // Verify the initial Nones are handled
        // RSI has 3 Nones.
        // StochRSI needs 3 valid RSIs, so it starts at index 3 + 3 - 1 = 5.
        // %K is SMA of StochRSI over 3, so it starts at index 5 + 3 - 1 = 7.
        // %D is SMA of %K over 3, so it starts at index 7 + 3 - 1 = 9.

        assert!(k_arr.get(6).is_none());
        assert!(k_arr.get(7).is_some()); // %K first valid at 7

        assert!(d_arr.get(8).is_none());
        assert!(d_arr.get(9).is_some()); // %D first valid at 9

        // Verify values are within bounds
        for i in 7..14 {
            if let Some(v) = k_arr.get(i) {
                assert!((0.0..=100.0).contains(&v), "%K out of bounds: {}", v);
            }
        }
        for i in 9..14 {
            if let Some(v) = d_arr.get(i) {
                assert!((0.0..=100.0).contains(&v), "%D out of bounds: {}", v);
            }
        }

        Ok(())
    }

    #[test]
    fn test_stoch_rsi_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14, 14, 3, 3);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_short = calculate(&df_short, 14, 14, 3, 3)?;
        let (k, d) = res_short;
        let k_arr = k.f64()?;
        let d_arr = d.f64()?;

        assert_eq!(k_arr.len(), 3);
        assert!(k_arr.get(2).is_none());
        assert!(d_arr.get(2).is_none());

        // Zero periods
        let res_zero = calculate(&df_short, 0, 14, 3, 3);
        assert!(res_zero.is_err());

        let res_zero_stoch = calculate(&df_short, 14, 0, 3, 3);
        assert!(res_zero_stoch.is_err());

        Ok(())
    }
}
