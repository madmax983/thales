//! ZLEMA - Zero Lag Exponential Moving Average
//!
//! Calculates the Zero Lag Exponential Moving Average (ZLEMA), a variation of the Exponential Moving Average (EMA) that aims to reduce the lag inherent in moving averages.
//! ZLEMA uses adjusted data (original data + its momentum over a specific lag period) before applying the EMA formula.
//!
//! # References
//! - [Investopedia - Zero Lag Exponential Moving Average (ZLEMA)](https://www.investopedia.com/terms/z/zlema.asp)

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Zero Lag Exponential Moving Average (ZLEMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with ZLEMA values. The first `period - 1` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let zlema = strategies::indicators::zlema::calculate(&df, 14)?;
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

    let lag = (period - 1) / 2;

    let period_dec = Decimal::from_usize(period).context("Invalid period")?;
    let k = Decimal::TWO / (period_dec + Decimal::ONE);

    let mut prev_ema: Option<Decimal> = None;
    let mut window_sum = Decimal::ZERO;
    let mut count = 0;

    let mut zlema_values: Vec<Option<f64>> = Vec::with_capacity(close.len());

    // Convert to Vec for O(1) indexed access since we need value at index `i` and `i - lag`
    let close_vec: Vec<Option<f64>> = close.into_iter().collect();

    for i in 0..close_vec.len() {
        if i < lag {
            zlema_values.push(None);
            continue;
        }

        let val_opt = close_vec[i];
        let lag_val_opt = close_vec[i - lag];

        match (val_opt, lag_val_opt) {
            (Some(val), Some(lag_val)) if val.is_finite() && lag_val.is_finite() => {
                let d = Decimal::from_f64_retain(val).unwrap_or(Decimal::ZERO);
                let lag_d = Decimal::from_f64_retain(lag_val).unwrap_or(Decimal::ZERO);

                // Adjusted Data = Close + (Close - Close[lag])
                let adj_data = d + (d - lag_d);

                if count < period {
                    window_sum += adj_data;
                    count += 1;

                    if count == period {
                        // Calculate SMA as seed
                        let seed = window_sum / period_dec;
                        zlema_values.push(seed.to_f64());
                        prev_ema = Some(seed);
                    } else {
                        zlema_values.push(None);
                    }
                } else {
                    // Calculate EMA
                    if let Some(prev) = prev_ema {
                        let ema = (adj_data * k) + (prev * (Decimal::ONE - k));
                        zlema_values.push(ema.to_f64());
                        prev_ema = Some(ema);
                    } else {
                        zlema_values.push(None);
                    }
                }
            }
            _ => {
                // Invalid or Missing value (NaN, None, etc.)
                zlema_values.push(None);
                count = 0;
                window_sum = Decimal::ZERO;
                prev_ema = None;
            }
        }
    }

    let s = Series::new("zlema", zlema_values);
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

        // Period 3. Lag = (3-1)/2 = 1.
        // i=0: val=10 -> lag_val=N/A -> None
        // i=1: val=11 -> lag_val=10 -> adj=11+(11-10)=12. count=1, sum=12. zlema=None
        // i=2: val=12 -> lag_val=11 -> adj=12+(12-11)=13. count=2, sum=25. zlema=None
        // i=3: val=13 -> lag_val=12 -> adj=13+(13-12)=14. count=3, sum=39. Seed=39/3=13.0. zlema=13.0
        // i=4: val=14 -> lag_val=13 -> adj=14+(14-13)=15. k=2/4=0.5. EMA=(15*0.5)+(13.0*0.5)=7.5+6.5=14.0
        // i=5: val=15 -> lag_val=14 -> adj=15+(15-14)=16. EMA=(16*0.5)+(14.0*0.5)=8+7=15.0
        // i=6: val=16 -> lag_val=15 -> adj=16+(16-15)=17. EMA=(17*0.5)+(15.0*0.5)=8.5+7.5=16.0

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());
        assert_eq!(out.get(3), Some(13.0));
        assert_eq!(out.get(4), Some(14.0));
        assert_eq!(out.get(5), Some(15.0));
        assert_eq!(out.get(6), Some(16.0));

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

        let result = calculate(&df, 5)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 10);
        // Lag = 2.
        // Needs 5 adjusted values -> starts outputting at index 2+4 = 6.
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());
        assert!(out.get(3).is_none());
        assert!(out.get(4).is_none());
        assert!(out.get(5).is_none());
        assert!(out.get(6).is_some());

        Ok(())
    }
}
