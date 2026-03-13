//! SMMA - Smoothed Moving Average
//!
//! The Smoothed Moving Average (SMMA) is an exponential moving average (EMA)
//! with a smoothing factor (alpha) equal to `1 / N`, where N is the period.
//! The first value is calculated as a Simple Moving Average (SMA).
//! SMMA gives recent prices equal weighting to historic prices as it takes
//! all available data into account, making it smoother than an EMA or SMA.
//!
//! # Example
//! ```rust
//! use polars::prelude::*;
//! use strategies::indicators::smma;
//!
//! // Assuming df is a DataFrame with a "close" column
//! // let df = ...;
//! // let result = smma::calculate(&df, 14)?;
//! ```

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Smoothed Moving Average (SMMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period (N)
///
/// # Returns
/// Series with SMMA values.
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

    let mut smma_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut prev_smma: Option<Decimal> = None;
    let mut window_sum = Decimal::ZERO;
    let mut count = 0;

    let period_dec = Decimal::from_usize(period).context("Invalid period")?;
    let n_minus_1 = period_dec - Decimal::ONE;

    for i in 0..close.len() {
        let val_opt = close.get(i);

        match val_opt {
            Some(val) => {
                if let Some(d) = Decimal::from_f64_retain(val) {
                    if count < period {
                        window_sum += d;
                        count += 1;

                        if count == period {
                            // Calculate SMA as seed
                            let seed = window_sum / period_dec;
                            smma_values.push(Some(
                                seed.to_f64().context("Failed to convert Decimal to f64")?,
                            ));
                            prev_smma = Some(seed);
                        } else {
                            smma_values.push(None);
                        }
                    } else {
                        // Calculate SMMA
                        if let Some(prev) = prev_smma {
                            // SMMA_i = (SMMA_{i-1} * (N - 1) + Close_i) / N
                            let smma = ((prev * n_minus_1) + d) / period_dec;
                            smma_values.push(Some(
                                smma.to_f64().context("Failed to convert Decimal to f64")?,
                            ));
                            prev_smma = Some(smma);
                        } else {
                            // Should not happen if logic is correct
                            smma_values.push(None);
                        }
                    }
                } else {
                    // Invalid value (NaN, etc.)
                    // Reset
                    smma_values.push(None);
                    count = 0;
                    window_sum = Decimal::ZERO;
                    prev_smma = None;
                }
            }
            None => {
                // Missing value
                smma_values.push(None);
                count = 0;
                window_sum = Decimal::ZERO;
                prev_smma = None;
            }
        }
    }

    let s = Series::new("smma", smma_values);
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

        // Period 3. Alpha = 1/3
        // 0: 10.0 -> count = 1 (sum 10.0)
        // 1: 11.0 -> count = 2 (sum 21.0)
        // 2: 12.0 -> count = 3 (sum 33.0). SMA(10,11,12) = 11.0. Seed.
        // 3: 13.0 -> SMMA_i = (SMMA_{i-1} * (N - 1) + Close_i) / N
        //                 = (11.0 * 2 + 13.0) / 3 = 35.0 / 3 = 11.666...
        // 4: 14.0 -> SMMA_i = (11.666... * 2 + 14.0) / 3 = 37.333... / 3 = 12.444...

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert_eq!(out.get(2), Some(11.0));

        if let Some(val3) = out.get(3) {
            assert!((val3 - 11.666666666666666).abs() < 1e-6);
        } else {
            anyhow::bail!("Expected value at index 3, found None");
        }

        if let Some(val4) = out.get(4) {
            assert!((val4 - 12.444444444444444).abs() < 1e-6);
        } else {
            anyhow::bail!("Expected value at index 4, found None");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        // Period > Data length
        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        // Period = 0
        let df_zero = df!("close" => &[10.0, 11.0])?;
        let res_zero = calculate(&df_zero, 0);
        assert!(res_zero.is_err());
        if let Err(e) = res_zero {
            assert_eq!(e.to_string(), "Period must be greater than 0");
        }

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[100.0, 102.0, 101.0, 103.0, 102.0]
        )?;

        // Period 2. N = 2.
        // i=0: count=1, sum=100.0
        // i=1: count=2, sum=202.0 -> SMA=101.0 (Seed)
        // i=2: close=101.0 -> SMMA = (101.0 * 1 + 101.0) / 2 = 101.0
        // i=3: close=103.0 -> SMMA = (101.0 * 1 + 103.0) / 2 = 102.0
        // i=4: close=102.0 -> SMMA = (102.0 * 1 + 102.0) / 2 = 102.0

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert_eq!(out.get(1), Some(101.0));
        assert_eq!(out.get(2), Some(101.0));
        assert_eq!(out.get(3), Some(102.0));
        assert_eq!(out.get(4), Some(102.0));

        Ok(())
    }
}
