//! Ulcer Index (UI)
//!
//! Calculates the Ulcer Index, a technical indicator that measures downside risk
//! in terms of both the depth and duration of price declines.
//!
//! It is typically calculated over a 14-period lookback.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal::MathematicalOps;
use std::collections::VecDeque;

/// Calculate Ulcer Index (UI)
///
/// # Arguments
/// * `data` - DataFrame with a "close" column
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

    let len = close.len();
    let mut ui_values: Vec<Option<f64>> = vec![None; len];

    if len < period {
        return Ok(Series::new("ulcer_index", ui_values));
    }

    let period_dec = Decimal::from_usize(period).context("Invalid period")?;
    let hundred = Decimal::new(100, 0);
    let mut close_vec: Vec<Option<Decimal>> = Vec::with_capacity(len);

    for i in 0..len {
        let val = close.get(i).and_then(Decimal::from_f64_retain);
        close_vec.push(val);
    }

    // Pass 1: Calculate drawdown squared for each day
    // A drawdown on day i is measured from the highest close in the past `period` days (including day i).
    // Let's first compute the rolling max of close over `period` days.
    let mut rolling_max: Vec<Option<Decimal>> = vec![None; len];
    let mut deque: VecDeque<usize> = VecDeque::new();

    for i in 0..len {
        if let Some(current_val) = close_vec[i] {
            // Remove indices outside the window [i - period + 1, i]
            while let Some(&front_idx) = deque.front() {
                if i >= period && front_idx <= i - period {
                    deque.pop_front();
                } else {
                    break;
                }
            }

            // Remove smaller elements from back
            while let Some(&back_idx) = deque.back() {
                if let Some(back_val) = close_vec[back_idx] {
                    if back_val <= current_val {
                        deque.pop_back();
                    } else {
                        break;
                    }
                } else {
                    deque.pop_back(); // Should not happen for valid values
                }
            }

            deque.push_back(i);
        }

        // If we have at least `period` elements, and the window has no missing values,
        // we can extract the rolling max.
        // Wait, what if there are missing values (None)?
        // For strictness, if any value in the window is None, the max is None.
        let mut valid = true;
        if i >= period - 1 {
            for j in 0..period {
                if close_vec[i - j].is_none() {
                    valid = false;
                    break;
                }
            }
        } else {
            valid = false;
        }

        if valid {
            if let Some(&max_idx) = deque.front() {
                rolling_max[i] = close_vec[max_idx];
            }
        }
    }

    // Now compute squared drawdown for each day
    let mut sq_drawdowns: Vec<Option<Decimal>> = vec![None; len];

    for i in 0..len {
        if let (Some(c), Some(max_c)) = (close_vec[i], rolling_max[i]) {
            if max_c > Decimal::ZERO {
                let drawdown = ((c - max_c) / max_c) * hundred;
                sq_drawdowns[i] = Some(drawdown * drawdown);
            }
        }
    }

    // Pass 2: Calculate Mean Squared Drawdown (MSD) over `period` days
    // UI = Sqrt(MSD)
    for i in 0..len {
        let mut valid_window = true;
        let mut local_sum = Decimal::ZERO;

        if i >= period - 1 {
            for j in 0..period {
                if let Some(val) = sq_drawdowns[i - j] {
                    local_sum += val;
                } else {
                    valid_window = false;
                    break;
                }
            }

            if valid_window {
                let msd = local_sum / period_dec;
                if let Some(ui_dec) = msd.sqrt() {
                    ui_values[i] = Some(ui_dec.to_f64().unwrap_or(0.0));
                } else {
                    ui_values[i] = Some(0.0);
                }
            }
        }
    }

    Ok(Series::new("ulcer_index", ui_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        // Data: 100, 105, 95, 90, 100
        // Period: 2
        // Rolling max (p=2):
        // i=0: None
        // i=1: max(100, 105) = 105
        // i=2: max(105, 95) = 105
        // i=3: max(95, 90) = 95
        // i=4: max(90, 100) = 100
        //
        // DD_j = (Close_j - RM_j)/RM_j * 100
        // DD_0: None
        // DD_1: (105-105)/105*100 = 0 -> Sq = 0
        // DD_2: (95-105)/105*100 = -9.5238 -> Sq = 90.7029
        // DD_3: (90-95)/95*100 = -5.2631 -> Sq = 27.7008
        // DD_4: (100-100)/100*100 = 0 -> Sq = 0
        //
        // UI_i (p=2 mean of Sq DDs):
        // UI_0: None
        // UI_1: None (needs DD_0 and DD_1, DD_0 is None)
        // UI_2: mean(DD_1^2, DD_2^2) = (0 + 90.7029)/2 = 45.3514 -> sqrt = 6.7343
        // UI_3: mean(DD_2^2, DD_3^2) = (90.7029 + 27.7008)/2 = 59.2018 -> sqrt = 7.6942
        // UI_4: mean(DD_3^2, DD_4^2) = (27.7008 + 0)/2 = 13.8504 -> sqrt = 3.7216

        let df = df!(
            "close" => &[100.0, 105.0, 95.0, 90.0, 100.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 5);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).unwrap();
        assert!(
            (val2 - 6.7343).abs() < 1e-3,
            "Expected ~6.7343, got {}",
            val2
        );

        let val3 = out.get(3).unwrap();
        assert!(
            (val3 - 7.6942).abs() < 1e-3,
            "Expected ~7.6942, got {}",
            val3
        );

        let val4 = out.get(4).unwrap();
        assert!(
            (val4 - 3.7216).abs() < 1e-3,
            "Expected ~3.7216, got {}",
            val4
        );

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[100.0, 101.0, 102.0])?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.f64()?;
        assert_eq!(out.len(), 3);
        assert!(out.get(0).is_none());
        assert!(out.get(2).is_none());

        let res_zero = calculate(&df_short, 0);
        assert!(res_zero.is_err());
        assert_eq!(res_zero.unwrap_err().to_string(), "Period must be greater than 0");

        let df_flat = df!("close" => &[100.0, 100.0, 100.0, 100.0])?;
        let res_flat = calculate(&df_flat, 2)?;
        let out_flat = res_flat.f64()?;
        assert!(out_flat.get(0).is_none());
        assert!(out_flat.get(1).is_none());
        assert_eq!(out_flat.get(2), Some(0.0));
        assert_eq!(out_flat.get(3), Some(0.0));

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 14);
        assert!(result.is_ok());
        let s = result?;
        assert_eq!(s.len(), 100);

        // Initial period values will be None
        let series = s.f64()?;

        // First valid sq_drawdown is at index 13.
        // First valid UI requires 14 valid sq_drawdowns, so index 13 + 13 = 26.
        assert!(series.get(25).is_none());
        assert!(series.get(26).is_some());

        for i in 26..100 {
            if let Some(v) = series.get(i) {
                assert!(
                    v >= 0.0,
                    "UI {} should be >= 0.0 at {}",
                    v,
                    i
                );
            }
        }

        Ok(())
    }
}
