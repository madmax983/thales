//! Aroon
//!
//! Calculates the Aroon indicator (Aroon Up and Aroon Down), which measures the time since the highest high and lowest low within a given period.
//!
//! Aroon Up = ((Period - Days Since Highest High) / Period) * 100
//! Aroon Down = ((Period - Days Since Lowest Low) / Period) * 100
//!
//! # Rationale
//! Aroon is used to identify the start of a new trend, the strength of a trend, and whether a stock is trending or trading sideways.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Aroon Indicator
///
/// # Arguments
/// * `data` - DataFrame with "high" and "low" columns
/// * `period` - Lookback period (typically 25)
///
/// # Returns
/// A tuple containing two Series: `(aroon_up, aroon_down)`
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let (aroon_up, aroon_down) = strategies::indicators::aroon::calculate(&df, 25)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<(Series, Series)> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let high_s = data.column("high").context("DataFrame must contain 'high' column")?;
    let low_s = data.column("low").context("DataFrame must contain 'low' column")?;

    let high_f64 = high_s.f64().context("High column must be numeric (f64)")?;
    let low_f64 = low_s.f64().context("Low column must be numeric (f64)")?;

    let len = high_f64.len();

    // We pre-allocate Vecs for Aroon Up and Aroon Down
    let mut aroon_up_values = vec![None; len];
    let mut aroon_down_values = vec![None; len];

    if len <= period {
        return Ok((
            Series::new("aroon_up", aroon_up_values),
            Series::new("aroon_down", aroon_down_values),
        ));
    }

    let period_dec = Decimal::from_usize(period).context("Invalid period")?;
    let hundred = Decimal::new(100, 0);

    for i in period..len {
        let start_idx = i - period;
        let end_idx = i;

        let mut highest_high: Option<Decimal> = None;
        let mut highest_idx = 0;

        let mut lowest_low: Option<Decimal> = None;
        let mut lowest_idx = 0;

        let mut valid_window = true;

        for j in start_idx..=end_idx {
            let h_val = high_f64.get(j);
            let l_val = low_f64.get(j);

            match (h_val, l_val) {
                (Some(h), Some(l)) => {
                    let h_dec = Decimal::from_f64_retain(h).unwrap_or(Decimal::ZERO);
                    let l_dec = Decimal::from_f64_retain(l).unwrap_or(Decimal::ZERO);

                    if let Some(hh) = highest_high {
                        if h_dec >= hh {
                            highest_high = Some(h_dec);
                            highest_idx = j;
                        }
                    } else {
                        highest_high = Some(h_dec);
                        highest_idx = j;
                    }

                    if let Some(ll) = lowest_low {
                        if l_dec <= ll {
                            lowest_low = Some(l_dec);
                            lowest_idx = j;
                        }
                    } else {
                        lowest_low = Some(l_dec);
                        lowest_idx = j;
                    }
                }
                _ => {
                    valid_window = false;
                    break;
                }
            }
        }

        if valid_window {
            // Days since highest high
            let days_since_high = i - highest_idx;
            let days_since_high_dec = Decimal::from_usize(days_since_high).context("Invalid days")?;

            // Aroon Up
            let aroon_up = ((period_dec - days_since_high_dec) / period_dec) * hundred;
            aroon_up_values[i] = aroon_up.to_f64();

            // Days since lowest low
            let days_since_low = i - lowest_idx;
            let days_since_low_dec = Decimal::from_usize(days_since_low).context("Invalid days")?;

            // Aroon Down
            let aroon_down = ((period_dec - days_since_low_dec) / period_dec) * hundred;
            aroon_down_values[i] = aroon_down.to_f64();
        }
    }

    Ok((
        Series::new("aroon_up", aroon_up_values),
        Series::new("aroon_down", aroon_down_values),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 15.0, 20.0, 18.0, 16.0, 25.0, 22.0],
            "low" =>  &[ 8.0, 12.0, 14.0, 15.0, 10.0,  9.0, 18.0]
        )?;

        let (up, down) = calculate(&df, 3)?;
        let up_vals = up.f64()?;
        let down_vals = down.f64()?;

        assert_eq!(up_vals.len(), 7);
        assert_eq!(down_vals.len(), 7);

        assert!(up_vals.get(0).is_none());
        assert!(up_vals.get(1).is_none());
        assert!(up_vals.get(2).is_none());

        let up_3 = up_vals.get(3).unwrap_or(0.0);
        let down_3 = down_vals.get(3).unwrap_or(100.0);
        assert!((up_3 - 66.6666).abs() < 1e-3, "Got {}", up_3);
        assert!((down_3 - 0.0).abs() < 1e-3, "Got {}", down_3);

        let up_4 = up_vals.get(4).unwrap_or(0.0);
        let down_4 = down_vals.get(4).unwrap_or(0.0);
        assert!((up_4 - 33.3333).abs() < 1e-3, "Got {}", up_4);
        assert!((down_4 - 100.0).abs() < 1e-3, "Got {}", down_4);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("high" => &[10.0, 11.0], "low" => &[9.0, 10.0])?;
        let res_short = calculate(&df_short, 5)?;
        let up_short = res_short.0.f64()?;
        assert_eq!(up_short.len(), 2);
        assert!(up_short.get(0).is_none());
        assert!(up_short.get(1).is_none());

        let res_zero = calculate(&df_short, 0);
        assert!(res_zero.is_err());
        assert_eq!(res_zero.unwrap_err().to_string(), "Period must be greater than 0");

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let highs: Vec<f64> = (0..50).map(|i| 100.0 + (i as f64).sin() * 5.0).collect();
        let lows: Vec<f64> = (0..50).map(|i| 90.0 + (i as f64).cos() * 5.0).collect();

        let df = df!("high" => highs, "low" => lows)?;
        let result = calculate(&df, 14);
        assert!(result.is_ok());

        let (up, down) = result?;
        assert_eq!(up.len(), 50);
        assert_eq!(down.len(), 50);

        let up_s = up.f64()?;
        let down_s = down.f64()?;

        assert!(up_s.get(13).is_none());
        assert!(up_s.get(14).is_some());

        for i in 14..50 {
            let u = up_s.get(i).unwrap_or(0.0);
            let d = down_s.get(i).unwrap_or(0.0);
            assert!((0.0..=100.0).contains(&u), "Aroon Up {} out of bounds", u);
            assert!((0.0..=100.0).contains(&d), "Aroon Down {} out of bounds", d);
        }

        Ok(())
    }
}
