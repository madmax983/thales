//! Mass Index - Identifies trend reversals by measuring the widening and narrowing of the range between high and low prices.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::indicators::ema;

/// Calculate Mass Index
///
/// # Arguments
/// * `data` - DataFrame with "high" and "low" columns
/// * `ema_period` - Lookback period for EMA (typically 9)
/// * `sum_period` - Lookback period for Mass Index sum (typically 25)
///
/// # Returns
/// Series with Mass Index values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::mass_index;
/// let df = DataFrame::new(vec![
///     Series::new("high", vec![10.0, 11.0, 12.0, 13.0, 14.0]),
///     Series::new("low", vec![9.0, 10.0, 11.0, 12.0, 13.0])
/// ]).unwrap();
/// // Note: actual usage requires enough data points to compute EMA and sum
/// // let result = mass_index::calculate(&df, 9, 25).unwrap();
/// ```
pub fn calculate(data: &DataFrame, ema_period: usize, sum_period: usize) -> Result<Series> {
    if data.is_empty() {
        anyhow::bail!("Data cannot be empty");
    }
    if ema_period == 0 {
        anyhow::bail!("EMA period must be greater than 0");
    }
    if sum_period == 0 {
        anyhow::bail!("Sum period must be greater than 0");
    }

    let high_f64 = data.column("high").context("DataFrame must contain 'high' column")?.f64()?;
    let low_f64 = data.column("low").context("DataFrame must contain 'low' column")?.f64()?;

    let mut hl_diff: Vec<Option<f64>> = Vec::with_capacity(high_f64.len());

    for (h_opt, l_opt) in high_f64.into_iter().zip(low_f64.into_iter()) {
        if let (Some(h), Some(l)) = (h_opt, l_opt) {
            if let (Some(h_dec), Some(l_dec)) = (Decimal::from_f64_retain(h), Decimal::from_f64_retain(l)) {
                let diff = h_dec - l_dec;
                hl_diff.push(diff.to_f64());
            } else {
                hl_diff.push(None);
            }
        } else {
            hl_diff.push(None);
        }
    }

    let diff_series = Series::new("close", hl_diff);
    let diff_df = DataFrame::new(vec![diff_series])?;

    let single_ema_series = ema::calculate(&diff_df, ema_period)?;
    let mut single_ema_renamed = single_ema_series.clone();
    single_ema_renamed.rename("close");
    let single_ema_df = DataFrame::new(vec![single_ema_renamed])?;

    let double_ema_series = ema::calculate(&single_ema_df, ema_period)?;

    let single_f64 = single_ema_series.f64()?;
    let double_f64 = double_ema_series.f64()?;

    let mut ema_ratio: Vec<Option<f64>> = Vec::with_capacity(single_f64.len());
    for (s_opt, d_opt) in single_f64.into_iter().zip(double_f64.into_iter()) {
        if let (Some(s), Some(d)) = (s_opt, d_opt) {
            if let (Some(s_dec), Some(d_dec)) = (Decimal::from_f64_retain(s), Decimal::from_f64_retain(d)) {
                if d_dec != Decimal::ZERO {
                    let ratio = s_dec / d_dec;
                    ema_ratio.push(ratio.to_f64());
                } else {
                    ema_ratio.push(None);
                }
            } else {
                ema_ratio.push(None);
            }
        } else {
            ema_ratio.push(None);
        }
    }

    let mut mass_index: Vec<Option<f64>> = Vec::with_capacity(ema_ratio.len());
    let mut window: std::collections::VecDeque<Decimal> = std::collections::VecDeque::with_capacity(sum_period);
    let mut current_sum = Decimal::ZERO;

    for val_opt in ema_ratio {
        if let Some(val) = val_opt {
            if let Some(d) = Decimal::from_f64_retain(val) {
                window.push_back(d);
                current_sum += d;

                if window.len() > sum_period {
                    if let Some(removed) = window.pop_front() {
                        current_sum -= removed;
                    }
                }

                if window.len() == sum_period {
                    mass_index.push(current_sum.to_f64());
                } else {
                    mass_index.push(None);
                }
            } else {
                window.clear();
                current_sum = Decimal::ZERO;
                mass_index.push(None);
            }
        } else {
            window.clear();
            current_sum = Decimal::ZERO;
            mass_index.push(None);
        }
    }

    Ok(Series::new("mass_index", mass_index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 9, 25);
        assert!(res_empty.is_err());

        let df_short = df!(
            "high" => &[10.0, 11.0],
            "low" => &[9.0, 10.0]
        )?;
        let result = calculate(&df_short, 9, 25)?;
        assert_eq!(result.len(), 2);

        Ok(())
    }

    #[test]
    fn test_known_values() -> Result<()> {
        // High - Low = 1.0 for all
        let size = 50;
        let mut high = Vec::with_capacity(size);
        let mut low = Vec::with_capacity(size);
        for _ in 0..size {
            high.push(10.0);
            low.push(9.0);
        }

        let df = df!(
            "high" => &high,
            "low" => &low
        )?;

        // EMA of 1.0 is 1.0. Double EMA is 1.0. Ratio is 1.0. Sum over 25 is 25.0
        let result = calculate(&df, 9, 25)?;
        let out = result.f64()?;

        assert_eq!(out.get(size - 1), Some(25.0));

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let size = 60;
        let mut high = Vec::with_capacity(size);
        let mut low = Vec::with_capacity(size);
        for i in 0..size {
            high.push(10.0 + (i as f64) * 0.1);
            low.push(9.0 + (i as f64) * 0.1);
        }

        let df = df!(
            "high" => &high,
            "low" => &low
        )?;

        let result = calculate(&df, 9, 25)?;
        let out = result.f64()?;

        let val = out.get(size - 1);
        assert!(val.is_some());

        Ok(())
    }
}
