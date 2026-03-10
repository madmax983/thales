use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Weighted Moving Average (WMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with WMA values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let result = strategies::indicators::wma::calculate(&df, 14)?;
/// ```
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

    let mut wma_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut window: VecDeque<Decimal> = VecDeque::with_capacity(period);

    // Sum of weights: (period * (period + 1)) / 2
    let sum_weights = Decimal::from_usize((period * (period + 1)) / 2)
        .context("Invalid period for Decimal conversion")?;

    for i in 0..close.len() {
        let val_opt = close.get(i);

        match val_opt {
            Some(val) => {
                if let Some(d) = Decimal::from_f64_retain(val) {
                    window.push_back(d);

                    if window.len() > period {
                        window.pop_front();
                    }

                    if window.len() == period {
                        let mut sum = Decimal::ZERO;
                        for (idx, price) in window.iter().enumerate() {
                            let weight = Decimal::from_usize(idx + 1)
                                .context("Invalid weight for Decimal conversion")?;
                            sum += price * weight;
                        }

                        let wma = sum / sum_weights;
                        wma_values.push(wma.to_f64());
                    } else {
                        wma_values.push(None);
                    }
                } else {
                    // Failed to convert (e.g. NaN)
                    wma_values.push(None);
                    window.clear();
                }
            }
            None => {
                // Missing data point
                wma_values.push(None);
                window.clear();
            }
        }
    }

    let s = Series::new("wma", wma_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
        )?;

        // Period 3 WMA
        // weights: 1, 2, 3
        // sum_weights: 6
        // i=0: None
        // i=1: None
        // i=2: (10*1 + 11*2 + 12*3) / 6 = (10 + 22 + 36) / 6 = 68 / 6 = 11.33333333
        // i=3: (11*1 + 12*2 + 13*3) / 6 = (11 + 24 + 39) / 6 = 74 / 6 = 12.33333333
        // i=4: (12*1 + 13*2 + 14*3) / 6 = (12 + 26 + 42) / 6 = 80 / 6 = 13.33333333

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).unwrap();
        assert!((val2 - 11.33333333).abs() < 1e-5);

        let val3 = out.get(3).unwrap();
        assert!((val3 - 12.33333333).abs() < 1e-5);

        let val4 = out.get(4).unwrap();
        assert!((val4 - 13.33333333).abs() < 1e-5);

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
        let out = res_short.f64()?;
        assert_eq!(out.len(), 2);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..50).map(|i| 100.0 + (i as f64 * 0.5)).collect();
        let df = df!("close" => values)?;

        let result = calculate(&df, 14)?;
        let s = result.f64()?;

        assert_eq!(s.len(), 50);
        assert!(s.get(12).is_none());
        assert!(s.get(13).is_some());
        assert!(s.get(49).is_some());

        Ok(())
    }
}