//! Simple Moving Average (SMA)
//!
//! A standard technical indicator that calculates the average of a selected range of prices,
//! usually closing prices, by the number of periods in that range.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Simple Moving Average (SMA)
///
/// # Arguments
/// * `data` - DataFrame with OHLCV data (must contain "close" column)
/// * `period` - Lookback period
///
/// # Returns
/// Series with SMA values (name: "sma_{period}")
///
/// # Implementation Notes
/// This implementation uses a manual loop with `rust_decimal::Decimal` for calculations
/// instead of Polars' vectorized `rolling_mean` (which uses `f64`).
/// This is to ensure precision and adhere to the "NO f64" requirement for financial calculations,
/// even though it sacrifices some performance.
///
/// The input is expected to be `Float64` (standard for market data) and is converted to `Decimal`.
/// The output is converted back to `Float64` Series for compatibility with Polars ecosystem.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.is_empty() {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let close = data.column("close")
        .context("DataFrame must have a 'close' column")?;

    // Assuming close is Float64.
    // If it's something else, we might need different handling, but f64 is standard for market data.
    let close_f64 = close.f64()
        .context("Close column must be Float64 type")?;

    let mut results: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut window: VecDeque<Decimal> = VecDeque::with_capacity(period);
    let mut sum = Decimal::ZERO;
    let period_dec = Decimal::from(period);

    for opt_val in close_f64.into_iter() {
        match opt_val {
            Some(v) => {
                 // Convert to Decimal, handling NaN/Inf by treating as break in data
                 match Decimal::from_f64_retain(v) {
                    Some(d) => {
                        window.push_back(d);
                        sum += d;

                        if window.len() > period {
                            let removed = window.pop_front().unwrap();
                            sum -= removed;
                        }

                        if window.len() == period {
                            let avg = sum / period_dec;
                            // Convert back to f64 for Series storage
                            results.push(avg.to_f64());
                        } else {
                            results.push(None);
                        }
                    },
                    None => {
                        // Invalid float (NaN/Inf) resets the window
                        window.clear();
                        sum = Decimal::ZERO;
                        results.push(None);
                    }
                 }
            },
            None => {
                // Missing data resets the window
                window.clear();
                sum = Decimal::ZERO;
                results.push(None);
            }
        }
    }

    let name = format!("sma_{}", period);
    Ok(Series::new(name.as_str().into(), results))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_df(closes: &[f64]) -> DataFrame {
        df!(
            "close" => closes,
        ).unwrap()
    }

    #[test]
    fn test_known_values() {
        // Example: 5-period SMA
        // Prices: 10, 11, 12, 13, 14, 15, 16
        let closes = vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0];
        let df = create_test_df(&closes);

        let result = calculate(&df, 5).unwrap();

        assert_eq!(result.len(), 7);
        assert_eq!(result.null_count(), 4);

        // 5th value (index 4): (10+11+12+13+14)/5 = 60/5 = 12.0
        let val4 = result.get(4).unwrap().extract::<f64>().unwrap();
        assert!((val4 - 12.0).abs() < 1e-10);

        // 6th value (index 5): (11+12+13+14+15)/5 = 65/5 = 13.0
        let val5 = result.get(5).unwrap().extract::<f64>().unwrap();
        assert!((val5 - 13.0).abs() < 1e-10);
    }

    #[test]
    fn test_edge_cases() {
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty, 5).is_err());

        let df_single = create_test_df(&[10.0]);
        let res = calculate(&df_single, 5).unwrap();
        assert_eq!(res.len(), 1);
        assert!(res.get(0).unwrap().is_null());

        // Test with None/NaN
        let closes = vec![10.0, 11.0, f64::NAN, 13.0, 14.0, 15.0];
        let df_nan = create_test_df(&closes);
        let res_nan = calculate(&df_nan, 3).unwrap();
        // Should handle NaN by resetting or treating as null, resulting in nulls
        // 10, 11, NaN -> null
        // 11, NaN, 13 -> null
        // NaN, 13, 14 -> null
        // 13, 14, 15 -> 14 (if it recovered)

        // My implementation clears window on NaN.
        // So:
        // 0: 10 (win: [10]) -> null
        // 1: 11 (win: [10, 11]) -> null
        // 2: NaN -> win cleared -> null
        // 3: 13 (win: [13]) -> null
        // 4: 14 (win: [13, 14]) -> null
        // 5: 15 (win: [13, 14, 15]) -> 14.0

        let last = res_nan.get(5).unwrap().extract::<f64>();
        assert!(last.is_some());
        assert!((last.unwrap() - 14.0).abs() < 1e-10);
    }

    #[test]
    fn test_realistic_data() {
         // Generate some data
         let mut closes = Vec::new();
         for i in 0..100 {
             closes.push(100.0 + (i as f64));
         }
         let df = create_test_df(&closes);
         let res = calculate(&df, 10).unwrap();

         // 10th value (index 9): 100..109 sum = 1045 / 10 = 104.5
         let val9 = res.get(9).unwrap().extract::<f64>().unwrap();
         assert!((val9 - 104.5).abs() < 1e-10);
    }
}
