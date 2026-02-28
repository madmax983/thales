use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Volume Weighted Moving Average (VWMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" and "volume" columns
/// * `period` - Lookback period
///
/// # Returns
/// Series with VWMA values.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Get "close" column
    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    // Get "volume" column
    let volume = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?
        .f64()
        .context("Volume column must be numeric (f64)")?;

    let mut vwma_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut window: VecDeque<(Decimal, Decimal)> = VecDeque::with_capacity(period);

    let mut sum_close_vol = Decimal::ZERO;
    let mut sum_vol = Decimal::ZERO;

    for i in 0..close.len() {
        let close_opt = close.get(i);
        let vol_opt = volume.get(i);

        match (close_opt, vol_opt) {
            (Some(c), Some(v)) => {
                if let (Some(c_dec), Some(v_dec)) =
                    (Decimal::from_f64_retain(c), Decimal::from_f64_retain(v))
                {
                    let cv_dec = c_dec * v_dec;

                    sum_close_vol += cv_dec;
                    sum_vol += v_dec;

                    window.push_back((cv_dec, v_dec));

                    if window.len() > period {
                        if let Some((old_cv, old_v)) = window.pop_front() {
                            sum_close_vol -= old_cv;
                            sum_vol -= old_v;
                        }
                    }

                    if window.len() == period {
                        if sum_vol == Decimal::ZERO {
                            // Avoid division by zero, though unlikely unless volume is exactly 0 for the whole period
                            vwma_values.push(Some(c)); // fallback to close price
                        } else {
                            // Check for safe division
                            let avg = sum_close_vol.checked_div(sum_vol).unwrap_or(Decimal::ZERO);
                            vwma_values.push(avg.to_f64());
                        }
                    } else {
                        vwma_values.push(None);
                    }
                } else {
                    vwma_values.push(None);
                    window.clear();
                    sum_close_vol = Decimal::ZERO;
                    sum_vol = Decimal::ZERO;
                }
            }
            _ => {
                // Missing data point
                vwma_values.push(None);
                window.clear();
                sum_close_vol = Decimal::ZERO;
                sum_vol = Decimal::ZERO;
            }
        }
    }

    let s = Series::new("vwma", vwma_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 10.0, 10.0, 10.0],
            "volume" => &[100.0, 200.0, 100.0, 200.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        // Period 2 VWMA:
        // i=0: None
        // i=1: (10*100 + 10*200) / 300 = 10.0
        // i=2: (10*200 + 10*100) / 300 = 10.0
        // i=3: (10*100 + 10*200) / 300 = 10.0

        assert!(out.get(0).is_none());
        assert_eq!(out.get(1), Some(10.0));
        assert_eq!(out.get(2), Some(10.0));
        assert_eq!(out.get(3), Some(10.0));

        Ok(())
    }

    #[test]
    fn test_varying_prices() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 20.0, 30.0],
            "volume" => &[100.0, 200.0, 300.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        // Period 2 VWMA:
        // i=0: None
        // i=1: (10*100 + 20*200) / 300 = 5000 / 300 = 16.6666...
        // i=2: (20*200 + 30*300) / 500 = 13000 / 500 = 26.0

        assert!(out.get(0).is_none());
        assert!((out.get(1).unwrap() - 16.66666666).abs() < 1e-5);
        assert_eq!(out.get(2), Some(26.0));

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Missing volume column
        let df_no_vol = df!("close" => &[10.0])?;
        let res_no_vol = calculate(&df_no_vol, 5);
        assert!(res_no_vol.is_err());

        Ok(())
    }
}
