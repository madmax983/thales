//! Fisher Transform
//!
//! Calculates the Fisher Transform, a technical indicator developed by John F. Ehlers
//! that converts price data into a Gaussian normal distribution.
//!
//! The indicator helps identify potential turning points and trends in asset prices.

use anyhow::Result;
use polars::prelude::*;

/// Calculate Fisher Transform
///
/// # Arguments
/// * `data` - DataFrame with "high" and "low" columns (or "close" if "high"/"low" are not present)
/// * `period` - Lookback period (standard is 9)
///
/// # Returns
/// Series with Fisher Transform values. The first `period - 1` values will be null.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let len = data.height();

    // We ideally want high/low prices, but fallback to close if they aren't available
    let (high, low) = if data.column("high").is_ok() && data.column("low").is_ok() {
        let h = data.column("high")?.f64()?;
        let l = data.column("low")?.f64()?;
        (h.clone(), l.clone())
    } else if let Ok(close_col) = data.column("close") {
        let c = close_col.f64()?;
        (c.clone(), c.clone())
    } else {
        anyhow::bail!("DataFrame must contain either 'high'/'low' or 'close' columns");
    };

    let mut fisher_values: Vec<Option<f64>> = vec![None; len];

    if len < period {
        return Ok(Series::new("fisher_transform", fisher_values));
    }

    let mut value1 = 0.0f64;
    let mut prev_fisher = 0.0f64;

    // Convert high/low series to vectors to iterate over windows
    let h_vec: Vec<Option<f64>> = high.into_iter().collect();
    let l_vec: Vec<Option<f64>> = low.into_iter().collect();

    for i in (period - 1)..len {
        let mut highest_high = f64::MIN;
        let mut lowest_low = f64::MAX;
        let mut valid_window = true;

        for j in 0..period {
            let h_opt = h_vec[i - j];
            let l_opt = l_vec[i - j];

            match (h_opt, l_opt) {
                (Some(h_val), Some(l_val)) => {
                    let h_f = if h_val.is_finite() { h_val } else { 0.0 };
                    let l_f = if l_val.is_finite() { l_val } else { 0.0 };

                    if h_f > highest_high {
                        highest_high = h_f;
                    }
                    if l_f < lowest_low {
                        lowest_low = l_f;
                    }
                }
                _ => {
                    valid_window = false;
                    break;
                }
            }
        }

        if !valid_window {
            fisher_values[i] = None;
            continue;
        }

        // Calculate median price for current period (typically (High + Low) / 2)
        // Here we just use the current period's median
        let h_val = h_vec[i].unwrap_or(0.0);
        let l_val = l_vec[i].unwrap_or(0.0);
        let curr_h = if h_val.is_finite() { h_val } else { 0.0 };
        let curr_l = if l_val.is_finite() { l_val } else { 0.0 };
        let price = (curr_h + curr_l) / 2.0;

        let range = highest_high - lowest_low;

        // Normalize the price to a value between -1 and 1
        let x = if range > 0.0 {
            let p_norm = (price - lowest_low) / range; // 0 to 1
            p_norm * 2.0 - 1.0 // -1 to 1
        } else {
            0.0
        };

        // Smooth value1
        let x_smoothed = 0.66 * x + 0.33 * value1;
        value1 = x_smoothed;

        // Truncate value1 to avoid infinities
        let limit = 0.999;
        let neg_limit = -limit;

        if value1 > limit {
            value1 = limit;
        } else if value1 < neg_limit {
            value1 = neg_limit;
        }

        // Calculate Fisher Transform
        // Formula: 0.5 * ln((1 + X) / (1 - X))
        let one = 1.0f64;
        let num = one + value1;
        let den = one - value1;

        let mut current_fisher = 0.0f64;
        if den > 0.0 {
            let ratio = num / den;

            if ratio > 0.0 {
                let ln_val = ratio.ln();

                current_fisher = 0.5 * ln_val + 0.5 * prev_fisher;
            }
        }

        fisher_values[i] = Some(current_fisher);
        prev_fisher = current_fisher;
    }

    Ok(Series::new("fisher_transform", fisher_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_fisher_transform() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 11.0, 12.0, 11.0, 10.0, 9.0, 8.0, 9.0, 10.0, 11.0],
            "low" => &[9.0, 10.0, 11.0, 10.0, 9.0, 8.0, 7.0, 8.0, 9.0, 10.0]
        )?;

        let result = calculate(&df, 4)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 10);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());
        assert!(out.get(3).is_some());

        Ok(())
    }
}
