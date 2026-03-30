//! Ulcer Index (UI)
//!
//! Calculates the Ulcer Index, a technical indicator that measures downside risk in terms of both the depth and duration of price declines.
//! The index increases in value as the price moves farther away from a recent high and falls as the price returns to new highs.
//!
//! Formula:
//! 1. Percentage Drawdown = [(Close - N-period High Close) / N-period High Close] * 100
//! 2. Squared Average = (N-period Sum of Percentage Drawdown Squared) / N
//! 3. Ulcer Index = Square Root of Squared Average

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate the Ulcer Index (UI)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period (typically 14)
///
/// # Returns
/// Series with Ulcer Index values. The first `period * 2 - 2` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::ulcer_index;
///
/// let df = df!(
///     "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
/// ).unwrap_or_default();
/// let result = ulcer_index::calculate(&df, 3).unwrap_or_default();
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let close_s = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .cast(&DataType::Float64)?;

    let close = close_s.f64().context("Close column must be numeric")?;

    let decimal_close: Vec<Option<Decimal>> = close
        .into_iter()
        .map(|opt_val| opt_val.and_then(Decimal::from_f64_retain))
        .collect();

    let mut ui_values: Vec<Option<f64>> = vec![None; decimal_close.len()];
    let mut drawdowns_sq: Vec<Option<Decimal>> = vec![None; decimal_close.len()];

    let hundred = Decimal::new(100, 0);
    let period_dec = Decimal::from_usize(period).context("Invalid period")?;

    // Step 1: Calculate Drawdowns Squared
    for i in 0..decimal_close.len() {
        if i + 1 < period {
            continue;
        }

        let mut max_close = Decimal::MIN;
        let mut valid = true;

        for j in 0..period {
            let idx = i + 1 - period + j;
            if let Some(val) = decimal_close[idx] {
                if val > max_close {
                    max_close = val;
                }
            } else {
                valid = false;
                break;
            }
        }

        if valid {
            if let Some(current_close) = decimal_close[i] {
                if !max_close.is_zero() {
                    let drawdown = ((current_close - max_close) / max_close) * hundred;
                    drawdowns_sq[i] = Some(drawdown * drawdown);
                } else {
                    drawdowns_sq[i] = Some(Decimal::ZERO);
                }
            }
        }
    }

    // Step 2 & 3: Calculate Squared Average and Ulcer Index
    for (i, ui_val) in ui_values.iter_mut().enumerate().take(drawdowns_sq.len()) {
        if i + 2 < period * 2 {
            continue;
        }

        let mut sum_sq = Decimal::ZERO;
        let mut valid = true;

        for j in 0..period {
            let idx = i + 1 - period + j;
            if let Some(sq) = drawdowns_sq[idx] {
                sum_sq += sq;
            } else {
                valid = false;
                break;
            }
        }

        if valid {
            let avg_sq = sum_sq / period_dec;
            if let Some(ui) = avg_sq.sqrt() {
                *ui_val = ui.to_f64();
            }
        }
    }

    let s = Series::new("ulcer_index", ui_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 10.0, 8.0, 8.0, 10.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).unwrap_or(0.0);
        assert!((val2 - 14.1421356).abs() < 1e-5);

        let val3 = out.get(3).unwrap_or(0.0);
        assert!((val3 - 14.1421356).abs() < 1e-5);

        let val4 = out.get(4).unwrap_or(-1.0);
        assert!((val4 - 0.0).abs() < 1e-5);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut closes = Vec::new();
        for i in 0..100 {
            closes.push(100.0 + (i as f64) * 0.5);
        }

        let df = df!("close" => &closes)?;
        let result = calculate(&df, 14)?;

        assert_eq!(result.len(), 100);
        let out = result.f64()?;

        assert!(out.get(25).is_none());
        assert!(out.get(26).is_some());
        assert!(out.get(99).is_some());

        Ok(())
    }
}
