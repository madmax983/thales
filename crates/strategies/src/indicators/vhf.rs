use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculates the Vertical Horizontal Filter (VHF).
///
/// VHF determines whether prices are in a trending phase or a congestion phase.
///
/// # Arguments
///
/// * `data` - DataFrame containing the "close" column.
/// * `period` - The lookback window (e.g., 28).
///
/// # Returns
///
/// A `Series` containing the VHF values. The first `period` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::vhf;
/// let df = df!("close" => &[10.0, 11.0, 12.0, 11.0, 10.0]).unwrap();
/// let result = vhf::calculate(&df, 3).unwrap();
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

    let mut vhf_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    // closes_window holds period + 1 prices, to compute `period` differences
    let mut closes_window: VecDeque<Decimal> = VecDeque::with_capacity(period + 1);
    let mut diffs_window: VecDeque<Decimal> = VecDeque::with_capacity(period);
    let mut sum_diffs = Decimal::ZERO;

    let mut prev_close: Option<Decimal> = None;

    for i in 0..close.len() {
        if let Some(val) = close.get(i) {
            if let Some(d) = Decimal::from_f64_retain(val) {
                closes_window.push_back(d);
                if closes_window.len() > period + 1 {
                    closes_window.pop_front();
                }

                if let Some(prev) = prev_close {
                    let diff = (d - prev).abs();
                    diffs_window.push_back(diff);
                    sum_diffs += diff;

                    if diffs_window.len() > period {
                        if let Some(old_diff) = diffs_window.pop_front() {
                            sum_diffs -= old_diff;
                        }
                    }
                }

                prev_close = Some(d);

                if diffs_window.len() == period && closes_window.len() == period + 1 {
                    // Safe because closes_window.len() == period + 1 > 0
                    let max_close = closes_window
                        .iter()
                        .copied()
                        .reduce(|a, b| a.max(b))
                        .unwrap_or(Decimal::ZERO);
                    let min_close = closes_window
                        .iter()
                        .copied()
                        .reduce(|a, b| a.min(b))
                        .unwrap_or(Decimal::ZERO);
                    let numerator = max_close - min_close;

                    if sum_diffs.is_zero() {
                        vhf_values.push(Some(0.0));
                    } else {
                        let vhf = numerator / sum_diffs;
                        vhf_values.push(vhf.to_f64());
                    }
                } else {
                    vhf_values.push(None);
                }
            } else {
                vhf_values.push(None);
                closes_window.clear();
                diffs_window.clear();
                sum_diffs = Decimal::ZERO;
                prev_close = None;
            }
        } else {
            vhf_values.push(None);
            closes_window.clear();
            diffs_window.clear();
            sum_diffs = Decimal::ZERO;
            prev_close = None;
        }
    }

    Ok(Series::new("vhf", vhf_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 12.0, 11.0, 13.0, 10.0, 9.0]
        )?;

        // Period 3
        let vhf = calculate(&df, 3)?;
        let out = vhf.f64()?;

        // VHF period 3 requires 3 price changes, meaning 4 prices:
        // i=0 (10.0): null
        // i=1 (12.0): null
        // i=2 (11.0): null
        // i=3 (13.0): prices [10, 12, 11, 13]. diffs: |12-10|=2, |11-12|=1, |13-11|=2. sum=5. max=13, min=10. num=3. VHF = 3/5 = 0.6
        // i=4 (10.0): prices [12, 11, 13, 10]. diffs: 1, 2, 3. sum=6. max=13, min=10. num=3. VHF = 3/6 = 0.5
        // i=5 (9.0): prices [11, 13, 10, 9]. diffs: 2, 3, 1. sum=6. max=13, min=9. num=4. VHF = 4/6 = 0.666...

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());

        let val3 = out.get(3).context("Expected value at index 3")?;
        assert!((val3 - 0.6).abs() < 1e-6, "Expected 0.6, got {}", val3);

        let val4 = out.get(4).context("Expected value at index 4")?;
        assert!((val4 - 0.5).abs() < 1e-6, "Expected 0.5, got {}", val4);

        let val5 = out.get(5).context("Expected value at index 5")?;
        assert!(
            (val5 - 0.6666666666666666).abs() < 1e-6,
            "Expected 0.666..., got {}",
            val5
        );

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());

        // Zero diff sum
        let df_zero = df!("close" => &[10.0, 10.0, 10.0, 10.0])?;
        let res_zero = calculate(&df_zero, 2)?;
        let out = res_zero.f64()?;
        assert_eq!(out.get(2), Some(0.0));

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[100.0, 102.0, 101.0, 103.0, 102.0, 105.0]
        )?;

        let s = calculate(&df, 3)?;
        let out = s.f64()?;

        assert!(out.get(2).is_none());
        assert!(out.get(3).is_some());
        assert!(out.get(4).is_some());
        assert!(out.get(5).is_some());

        Ok(())
    }
}
