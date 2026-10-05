//! Ulcer Index (UI)
//!
//! Measures downside risk by calculating the depth and duration of price drawdowns.

use anyhow::{Context, Result};
use polars::prelude::*;
use std::collections::VecDeque;

/// Calculate Ulcer Index
///
/// # Arguments
/// * `data` - DataFrame with a "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with Ulcer Index values
///
/// # Example
/// ```rust
/// use strategies::indicators::ulcer_index;
/// use polars::prelude::*;
/// // let mut df = df!("close" => &[10.0, 11.0]).unwrap();
/// // let result = ulcer_index::calculate(&df, 14);
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
        .context("DataFrame must contain 'close' column")?;
    let close_arr = close
        .f64()
        .context("'close' column must be numeric (f64)")?;

    let mut ui_values: Vec<Option<f64>> = Vec::with_capacity(close_arr.len());
    let mut max_window: VecDeque<f64> = VecDeque::with_capacity(period);
    let mut dd_window: VecDeque<f64> = VecDeque::with_capacity(period);

    let period_f = period as f64;

    for i in 0..close_arr.len() {
        match close_arr.get(i).filter(|v| v.is_finite()) {
            Some(d) => {
                max_window.push_back(d);
                if max_window.len() > period {
                    max_window.pop_front();
                }

                let current_max = max_window.iter().fold(0.0_f64, |m, &v| m.max(v));

                let drawdown = if current_max > 0.0 {
                    (d - current_max) / current_max * 100.0
                } else {
                    0.0
                };

                dd_window.push_back(drawdown);
                if dd_window.len() > period {
                    dd_window.pop_front();
                }

                if dd_window.len() == period {
                    let sum_sq_dd: f64 = dd_window.iter().map(|&dd| dd * dd).sum();
                    ui_values.push(Some((sum_sq_dd / period_f).sqrt()));
                } else {
                    ui_values.push(None);
                }
            }
            None => {
                max_window.clear();
                dd_window.clear();
                ui_values.push(None);
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
    fn test_ulcer_index_known_values() -> Result<()> {
        // Values: 10, 9, 8
        // Period 3
        // Day 1: close=10. max(10)=10. dd=(10-10)/10 * 100 = 0.
        // Day 2: close=9. max(10, 9)=10. dd=(9-10)/10 * 100 = -10.
        // Day 3: close=8. max(10, 9, 8)=10. dd=(8-10)/10 * 100 = -20.
        // Sum of sq drawdowns = 0^2 + (-10)^2 + (-20)^2 = 0 + 100 + 400 = 500.
        // Avg sq = 500 / 3 = 166.666...
        // UI = sqrt(166.666...) = 12.909944...

        let mut df = df!("close" => &[10, 9, 8, 7, 6])?;
        df.try_apply("close", |s| s.cast(&DataType::Float64))?;
        let result = calculate(&df, 3)?;
        let ui_arr = result.f64()?;

        assert!(ui_arr.get(0).is_none());
        assert!(ui_arr.get(1).is_none());

        if let Some(val_2) = ui_arr.get(2) {
            assert!((val_2 - 12.909944).abs() < 0.0001);
        } else {
            anyhow::bail!("Expected value at index 2");
        }

        // Day 4: close=7.
        // max_window = [9, 8, 7] => max = 9.
        // dd_window[0] = -10 (from day 2, max was 10)
        // dd_window[1] = -20 (from day 3, max was 10)
        // dd_window[2] = (7-9)/9 * 100 = -22.222... (max is 9 for day 4)
        // Sum_sq = (-10)^2 + (-20)^2 + (-22.222)^2 = 100 + 400 + 493.827 = 993.827
        // Avg_sq = 993.827 / 3 = 331.275
        // UI = sqrt(331.275) = 18.2009

        if let Some(val_3) = ui_arr.get(3) {
            assert!(
                (val_3 - 18.2009).abs() < 0.001,
                "Expected ~18.2009 but got {}",
                val_3
            );
        } else {
            anyhow::bail!("Expected value at index 3");
        }

        Ok(())
    }

    #[test]
    fn test_ulcer_index_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());

        let mut df_short = df!("close" => &[10, 11])?;
        df_short.try_apply("close", |s| s.cast(&DataType::Float64))?;
        let res_short = calculate(&df_short, 14)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_ulcer_index_realistic_data() -> Result<()> {
        let values: Vec<i32> = (0..100).map(|i| 100 + (i % 10)).collect();
        let mut df = df!("close" => values)?;
        df.try_apply("close", |s| s.cast(&DataType::Float64))?;
        let res = calculate(&df, 14)?;

        assert_eq!(res.len(), 100);
        let arr = res.f64()?;
        assert!(arr.get(12).is_none());
        assert!(arr.get(13).is_some());

        Ok(())
    }
}
