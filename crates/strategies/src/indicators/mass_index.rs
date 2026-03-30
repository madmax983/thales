//! Mass Index
//!
//! Calculates the Mass Index, an indicator designed by Donald Dorsey to identify trend reversals
//! by measuring the narrowing and widening of the range between high and low prices.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

use super::ema;

/// Calculate Mass Index
///
/// # Arguments
/// * `data` - DataFrame with "high" and "low" columns
/// * `ema_period` - Lookback period for the EMAs (standard is 9)
/// * `sum_period` - Lookback period for the rolling sum (standard is 25)
///
/// # Returns
/// Series with Mass Index values.
///
/// # Example
/// ```rust
/// use anyhow::Result;
/// use polars::prelude::*;
/// use strategies::indicators::mass_index;
///
/// fn example() -> Result<()> {
///     let df = df!(
///         "high" => &[10.0, 11.0, 12.0, 13.0, 14.0],
///         "low"  => &[9.0, 10.0, 11.0, 12.0, 13.0]
///     )?;
///     // Usually requires enough data (e.g., ema_period*2 + sum_period) to produce values
///     let mass_idx_series = mass_index::calculate(&df, 9, 25)?;
///     Ok(())
/// }
/// ```
pub fn calculate(data: &DataFrame, ema_period: usize, sum_period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if ema_period == 0 || sum_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

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

    let len = high.len();

    // 1. Calculate High - Low natively but convert to Decimal inside the loop to avoid Polars f64 math
    // Instead of using polars f64 arithmetic, we extract arrays and do Decimal math iteratively.
    let mut tr_values: Vec<Option<f64>> = Vec::with_capacity(len);
    for i in 0..len {
        let h_opt = high.get(i);
        let l_opt = low.get(i);
        if let (Some(h), Some(l)) = (h_opt, l_opt) {
            let h_dec = Decimal::from_f64_retain(h);
            let l_dec = Decimal::from_f64_retain(l);
            if let (Some(hd), Some(ld)) = (h_dec, l_dec) {
                let diff = hd - ld;
                // Safely extract the f64 representation for intermediate DataFrame steps
                // as `ema::calculate` will correctly recast back to Decimal internally
                if let Some(val) = diff.to_f64() {
                    tr_values.push(Some(val));
                } else {
                    tr_values.push(None);
                }
            } else {
                tr_values.push(None);
            }
        } else {
            tr_values.push(None);
        }
    }

    let mut tr_series = Series::new("tr", tr_values);
    let temp_df = DataFrame::new(vec![tr_series.rename("close").clone()])?;

    // 2. Single EMA of (High - Low)
    let mut single_ema_series = ema::calculate(&temp_df, ema_period)?;

    // 3. Double EMA of (High - Low)
    let temp_df2 = DataFrame::new(vec![single_ema_series.rename("close").clone()])?;
    let double_ema_series = ema::calculate(&temp_df2, ema_period)?;

    let single_ema_f64 = single_ema_series.f64()?;
    let double_ema_f64 = double_ema_series.f64()?;

    // 4. EMA Ratio = Single EMA / Double EMA
    let mut ema_ratio: Vec<Option<Decimal>> = Vec::with_capacity(len);
    for i in 0..len {
        let s_opt = single_ema_f64.get(i);
        let d_opt = double_ema_f64.get(i);

        if let (Some(s), Some(d)) = (s_opt, d_opt) {
            let s_dec = Decimal::from_f64_retain(s);
            let d_dec = Decimal::from_f64_retain(d);

            if let (Some(sd), Some(dd)) = (s_dec, d_dec) {
                if !dd.is_zero() {
                    ema_ratio.push(Some(sd / dd));
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

    // 5. Mass Index = Rolling sum of EMA Ratio over `sum_period`
    let mut mass_index_values: Vec<Option<f64>> = vec![None; len];
    let mut window: VecDeque<Decimal> = VecDeque::with_capacity(sum_period);
    let mut sum = Decimal::ZERO;

    for i in 0..len {
        if let Some(ratio) = ema_ratio[i] {
            sum += ratio;
            window.push_back(ratio);

            if window.len() > sum_period {
                if let Some(old) = window.pop_front() {
                    sum -= old;
                }
            }

            if window.len() == sum_period {
                if let Some(val) = sum.to_f64() {
                    mass_index_values[i] = Some(val);
                } else {
                    mass_index_values[i] = None;
                }
            } else {
                mass_index_values[i] = None;
            }
        } else {
            // Missing data propagates
            window.clear();
            sum = Decimal::ZERO;
            mass_index_values[i] = None;
        }
    }

    Ok(Series::new("mass_index", mass_index_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0],
            "low"  => &[8.0,  10.0, 12.0, 14.0, 16.0, 18.0, 20.0]
        )?;

        // Period sum=2, ema=2
        // TR = [2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0]
        // Since TR is constant, EMA(TR) = 2.0 eventually, Double EMA(TR) = 2.0 eventually.
        // Ratio = 1.0.
        // Sum(Ratio) over 2 periods = 2.0

        let result = calculate(&df, 2, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 7);
        // Initial warmup for EMA and Sum
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        // Eventually reaches 2.0
        let last_val = out.get(6).context("Missing value at index 6")?;
        assert!(
            (last_val - 2.0).abs() < 1e-4,
            "Expected ~2.0, got {}",
            last_val
        );

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 9, 25);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period = 0
        let df_normal = df!("high" => &[10.0, 11.0], "low" => &[9.0, 10.0])?;
        let res_zero = calculate(&df_normal, 0, 25);
        assert!(res_zero.is_err());
        assert_eq!(res_zero.unwrap_err().to_string(), "Periods must be greater than 0");

        // Data length shorter than needed for warmup
        let df_short = df!("high" => &[10.0, 11.0], "low" => &[9.0, 10.0])?;
        let res_short = calculate(&df_short, 9, 25)?;
        let out = res_short.f64()?;
        assert_eq!(out.len(), 2);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        // Zero division protection
        let df_flat = df!("high" => &[10.0, 10.0, 10.0, 10.0, 10.0], "low" => &[10.0, 10.0, 10.0, 10.0, 10.0])?;
        let res_flat = calculate(&df_flat, 2, 2)?;
        let out_flat = res_flat.f64()?;
        // TR is 0, EMA is 0, Double EMA is 0. Ratio involves division by 0.
        // Should produce None gracefully and not panic.
        for i in 0..5 {
            assert!(out_flat.get(i).is_none());
        }

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let highs: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0 + 2.0)
            .collect();
        let lows: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0 - 2.0)
            .collect();

        let df = df!("high" => highs, "low" => lows)?;
        let ema_period = 9;
        let sum_period = 25;

        let result = calculate(&df, ema_period, sum_period);
        assert!(result.is_ok());

        let s = result?;
        assert_eq!(s.len(), 100);

        let series = s.f64()?;

        // Ensure initial values are None
        assert!(series.get(0).is_none());

        // Eventually produces values
        let last_val = series.get(99).context("Missing value at index 99")?;
        // Since TR is roughly constant (4.0), EMA and Double EMA should converge to 4.0
        // Ratio should converge to 1.0. Sum over 25 periods should be ~25.0
        assert!(
            (last_val - 25.0).abs() < 1.0,
            "Mass Index {} out of expected bounds at end of data", last_val
        );

        Ok(())
    }
}
