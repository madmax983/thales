//! Aroon Oscillator - A trend-following indicator that uses Aroon Up and Aroon Down.
//!
//! Aroon Up measures the number of periods since the highest high within the period.
//! Aroon Down measures the number of periods since the lowest low within the period.
//! The oscillator is the difference between Up and Down.

use anyhow::{Context, Result};
use polars::prelude::*;

/// Calculate Aroon Oscillator
///
/// # Arguments
/// * `data` - DataFrame with "high" and "low" columns.
/// * `period` - Lookback period for Aroon calculation.
///
/// # Returns
/// Tuple of (Aroon Up, Aroon Down, Aroon Oscillator) Series.
pub fn calculate(data: &DataFrame, period: usize) -> Result<(Series, Series, Series)> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
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

    let highs_vec: Vec<Option<f64>> = high.into_iter().collect();
    let lows_vec: Vec<Option<f64>> = low.into_iter().collect();

    let mut aroon_up_vals = Vec::with_capacity(high.len());
    let mut aroon_down_vals = Vec::with_capacity(low.len());
    let mut aroon_osc_vals = Vec::with_capacity(high.len());

    for i in 0..high.len() {
        if i < period {
            aroon_up_vals.push(None);
            aroon_down_vals.push(None);
            aroon_osc_vals.push(None);
            continue;
        }

        // Lookback window is from (i - period) to i inclusive, so window size is period + 1.
        let window_start = i - period;
        let mut max_high = f64::NEG_INFINITY;
        let mut max_idx = 0;
        let mut min_low = f64::INFINITY;
        let mut min_idx = 0;
        let mut valid_window = true;

        for j in window_start..=i {
            if let Some(h) = highs_vec[j] {
                // If there are multiple highs with the same value, the most recent one is used
                // so we use `>=` instead of `>`.
                if h >= max_high {
                    max_high = h;
                    max_idx = j;
                }
            } else {
                valid_window = false;
                break;
            }

            if let Some(l) = lows_vec[j] {
                // Similarly for lowest low, the most recent is used.
                if l <= min_low {
                    min_low = l;
                    min_idx = j;
                }
            } else {
                valid_window = false;
                break;
            }
        }

        if valid_window {
            // Periods since highest high / lowest low
            let periods_since_high = i - max_idx;
            let periods_since_low = i - min_idx;

            let aroon_up = 100.0 * ((period as f64 - periods_since_high as f64) / period as f64);
            let aroon_down = 100.0 * ((period as f64 - periods_since_low as f64) / period as f64);
            let aroon_osc = aroon_up - aroon_down;

            aroon_up_vals.push(Some(aroon_up));
            aroon_down_vals.push(Some(aroon_down));
            aroon_osc_vals.push(Some(aroon_osc));
        } else {
            aroon_up_vals.push(None);
            aroon_down_vals.push(None);
            aroon_osc_vals.push(None);
        }
    }

    let s_aroon_up = Series::new("aroon_up", aroon_up_vals);
    let s_aroon_down = Series::new("aroon_down", aroon_down_vals);
    let s_aroon_osc = Series::new("aroon_oscillator", aroon_osc_vals);

    Ok((s_aroon_up, s_aroon_down, s_aroon_osc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_aroon_calculation() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 15.0, 12.0, 18.0, 20.0, 19.0, 17.0, 21.0],
            "low" =>  &[ 5.0,  8.0,  6.0, 10.0, 15.0, 14.0, 12.0, 16.0]
        )?;

        let period = 3;
        let (aroon_up, aroon_down, aroon_osc) = calculate(&df, period)?;

        let up = aroon_up.f64()?;
        let down = aroon_down.f64()?;
        let osc = aroon_osc.f64()?;

        // i = 0,1,2: None because period=3 means we need indices [0,1,2,3] for i=3
        assert!(up.get(0).is_none());
        assert!(up.get(1).is_none());
        assert!(up.get(2).is_none());

        // i=3: window indices [0..=3]
        // highs: 10, 15, 12, 18 -> max is 18 at idx 3. periods_since = 0 -> up = 100 * (3-0)/3 = 100
        // lows: 5, 8, 6, 10 -> min is 5 at idx 0. periods_since = 3 -> down = 100 * (3-3)/3 = 0
        // osc = 100 - 0 = 100
        assert_eq!(up.get(3), Some(100.0));
        assert_eq!(down.get(3), Some(0.0));
        assert_eq!(osc.get(3), Some(100.0));

        // i=4: window [1..=4]
        // highs: 15, 12, 18, 20 -> max 20 at idx 4. periods_since=0 -> up=100
        // lows: 8, 6, 10, 15 -> min 6 at idx 2. periods_since=2 -> down = 100 * (3-2)/3 = 33.333
        assert_eq!(up.get(4), Some(100.0));
        assert!((down.get(4).unwrap() - 33.3333).abs() < 0.001);

        Ok(())
    }
}
