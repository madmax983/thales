use anyhow::{Context, Result};
use polars::prelude::*;
use std::collections::VecDeque;

/// Calculate Qstick indicator
///
/// # Arguments
/// * `data` - DataFrame with "open" and "close" columns
/// * `period` - Lookback period for SMA of (Close - Open)
///
/// # Returns
/// Series with Qstick values.
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

    let open = data
        .column("open")
        .context("DataFrame must contain 'open' column")?
        .f64()
        .context("Open column must be numeric (f64)")?;

    let mut qstick_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut window: VecDeque<f64> = VecDeque::with_capacity(period);
    let mut sum = 0.0f64;
    let period_f = period as f64;

    for i in 0..close.len() {
        let close_opt = close.get(i);
        let open_opt = open.get(i);

        match (close_opt, open_opt) {
            (Some(c), Some(o)) if c.is_finite() && o.is_finite() => {
                let diff = c - o;
                sum += diff;
                window.push_back(diff);

                if window.len() > period {
                    if let Some(old) = window.pop_front() {
                        sum -= old;
                    }
                }

                if window.len() == period {
                    let avg = sum / period_f;
                    qstick_values.push(Some(avg));
                } else {
                    qstick_values.push(None);
                }
            }
            _ => {
                qstick_values.push(None);
                window.clear();
                sum = 0.0;
            }
        }
    }

    let s = Series::new("qstick", qstick_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_qstick_calculation() -> Result<()> {
        let df = df!(
            "open" => &[10.0, 11.0, 12.0, 13.0, 14.0],
            "close" => &[11.0, 10.0, 13.0, 15.0, 13.0]
        )?;

        // period = 2
        // diff: [1.0, -1.0, 1.0, 2.0, -1.0]
        // sum(0,1) = 0.0 -> qstick[1] = 0.0
        // sum(1,2) = 0.0 -> qstick[2] = 0.0
        // sum(2,3) = 3.0 -> qstick[3] = 1.5
        // sum(3,4) = 1.0 -> qstick[4] = 0.5

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert_eq!(out.get(1), Some(0.0));
        assert_eq!(out.get(2), Some(0.0));
        assert_eq!(out.get(3), Some(1.5));
        assert_eq!(out.get(4), Some(0.5));

        Ok(())
    }
}
