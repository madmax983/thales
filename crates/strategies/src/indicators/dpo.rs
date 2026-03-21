//! DPO - Detrended Price Oscillator
//!
//! Calculates the Detrended Price Oscillator (DPO), an indicator designed to remove trend
//! from price and make it easier to identify cycles.
//! DPO does not extend to the last date because it is based on a displaced moving average.
//!
//! # References
//! - [Investopedia - Detrended Price Oscillator (DPO)](https://www.investopedia.com/terms/d/detrended-price-oscillator-dpo.asp)

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Detrended Price Oscillator (DPO)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period for the moving average (typically 20 or 30)
///
/// # Returns
/// Series with DPO values.
///
/// # Formula
/// DPO = Close - SMA(Close, period) displaced back by (period / 2) + 1 days.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let dpo = strategies::indicators::dpo::calculate(&df, 20)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
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

    let close_vec: Vec<Option<f64>> = close.into_iter().collect();
    let n = close_vec.len();

    // Displacement back is (period / 2) + 1.
    // e.g. for period=20, displace back by 10+1 = 11.
    let displacement = (period / 2) + 1;

    let period_dec = Decimal::from_usize(period).context("Invalid period")?;

    // Precalculate SMA
    let mut sma_values: Vec<Option<Decimal>> = vec![None; n];
    let mut window_sum = Decimal::ZERO;
    let mut count = 0;

    for i in 0..n {
        let val_opt = close_vec[i];
        if let Some(val) = val_opt {
            let d = Decimal::from_f64_retain(val).context("Failed to parse close as Decimal")?;
            window_sum += d;
            count += 1;

            if count > period {
                if let Some(old_val) = close_vec[i - period] {
                    window_sum -= Decimal::from_f64_retain(old_val)
                        .context("Failed to parse close as Decimal")?;
                }
                count -= 1;
            }

            if count == period {
                sma_values[i] = Some(window_sum / period_dec);
            }
        } else {
            // Gap in data
            count = 0;
            window_sum = Decimal::ZERO;
        }
    }

    // Calculate DPO
    // DPO[i] = Close[i] - SMA[i + displacement]

    let mut dpo_values: Vec<Option<f64>> = vec![None; n];

    for i in 0..n {
        let sma_idx = i + displacement;
        if sma_idx < n {
            if let (Some(close_val), Some(sma_val)) = (close_vec[i], sma_values[sma_idx]) {
                let close_dec = Decimal::from_f64_retain(close_val)
                    .context("Failed to parse close as Decimal")?;
                let dpo = close_dec - sma_val;
                dpo_values[i] = Some(
                    dpo.to_f64()
                        .context("Failed to convert DPO Decimal back to f64")?,
                );
            }
        }
    }

    let s = Series::new("dpo", dpo_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0]
        )?;

        // Period 4. Displacement = 4/2 + 1 = 3.
        // SMA values (period=4):
        // i=3: sum=46, SMA=11.5
        // i=4: sum=50, SMA=12.5
        // i=5: sum=54, SMA=13.5
        // i=6: sum=58, SMA=14.5

        // DPO[i] = Close[i] - SMA[i+3]
        // i=0: Close=10.0, SMA[3]=11.5 -> DPO = -1.5
        // i=1: Close=11.0, SMA[4]=12.5 -> DPO = -1.5
        // i=2: Close=12.0, SMA[5]=13.5 -> DPO = -1.5
        // i=3: Close=13.0, SMA[6]=14.5 -> DPO = -1.5
        // i=4,5,6: SMA idx out of bounds -> None

        let result = calculate(&df, 4)?;
        let out = result.f64()?;

        assert_eq!(out.get(0), Some(-1.5));
        assert_eq!(out.get(1), Some(-1.5));
        assert_eq!(out.get(2), Some(-1.5));
        assert_eq!(out.get(3), Some(-1.5));
        assert!(out.get(4).is_none());
        assert!(out.get(5).is_none());
        assert!(out.get(6).is_none());

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
        // period=5. SMA needs 5 points. Will be None everywhere.
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[
                100.0, 102.0, 101.0, 104.0, 107.0, 105.0, 108.0, 110.0, 109.0, 112.0
            ]
        )?;

        // Period 6. Displacement = 3 + 1 = 4.
        let result = calculate(&df, 6)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 10);
        // SMA start at index 5.
        // DPO[i] needs SMA[i+4].
        // i=0 needs SMA[4] (None)
        // i=1 needs SMA[5] (Some) -> DPO[1] is Some
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_some());
        assert!(out.get(5).is_some()); // SMA[9] exists
        assert!(out.get(6).is_none()); // SMA[10] out of bounds

        Ok(())
    }
}
