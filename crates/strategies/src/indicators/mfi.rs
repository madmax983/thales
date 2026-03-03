//! Money Flow Index (MFI)
//!
//! Calculates the Money Flow Index (MFI), a volume-weighted oscillator that measures buying and selling pressure.
//! MFI oscillates between 0 and 100.
//!
//! Traditionally, MFI is considered overbought when above 80 and oversold when below 20.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Money Flow Index (MFI)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close", "volume" columns
/// * `period` - Lookback period (standard is 14)
///
/// # Returns
/// Series with MFI values. The first `period` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let mfi = strategies::indicators::mfi::calculate(&df, 14)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let high = data
        .column("high")
        .context("DataFrame must contain 'high' column")?
        .f64()
        .context("High column must be numeric (f64)")?;
    let low = data
        .column("low")
        .context("DataFrame must contain 'low' column")?
        .f64()
        .context("Low column must be numeric (f64)")?;
    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;
    let volume = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?
        .f64()
        .context("Volume column must be numeric (f64)")?;

    let len = close.len();
    let mut mfi_values: Vec<Option<f64>> = vec![None; len];

    if len <= period {
        return Ok(Series::new("mfi", mfi_values));
    }

    let three = Decimal::new(3, 0);
    let hundred = Decimal::new(100, 0);

    let mut typical_prices: Vec<Option<Decimal>> = Vec::with_capacity(len);

    for i in 0..len {
        let h_opt = high.get(i);
        let l_opt = low.get(i);
        let c_opt = close.get(i);

        if let (Some(h), Some(l), Some(c)) = (h_opt, l_opt, c_opt) {
            let h_dec = Decimal::from_f64_retain(h).unwrap_or(Decimal::ZERO);
            let l_dec = Decimal::from_f64_retain(l).unwrap_or(Decimal::ZERO);
            let c_dec = Decimal::from_f64_retain(c).unwrap_or(Decimal::ZERO);
            typical_prices.push(Some((h_dec + l_dec + c_dec) / three));
        } else {
            typical_prices.push(None);
        }
    }

    let mut raw_money_flows: Vec<Option<Decimal>> = Vec::with_capacity(len);
    for i in 0..len {
        let tp_opt = typical_prices[i];
        let v_opt = volume.get(i);

        if let (Some(tp), Some(v)) = (tp_opt, v_opt) {
            let v_dec = Decimal::from_f64_retain(v).unwrap_or(Decimal::ZERO);
            raw_money_flows.push(Some(tp * v_dec));
        } else {
            raw_money_flows.push(None);
        }
    }

    let mut pos_flows: Vec<Decimal> = vec![Decimal::ZERO; len];
    let mut neg_flows: Vec<Decimal> = vec![Decimal::ZERO; len];

    for i in 1..len {
        let curr_tp = typical_prices[i];
        let prev_tp = typical_prices[i - 1];
        let rmf = raw_money_flows[i];

        if let (Some(curr), Some(prev), Some(flow)) = (curr_tp, prev_tp, rmf) {
            if curr > prev {
                pos_flows[i] = flow;
            } else if curr < prev {
                neg_flows[i] = flow;
            }
        }
    }

    // Now calculate MFI over the period window
    // MFI requires summing the positive and negative flows over the `period` length
    let mut current_pos_sum = Decimal::ZERO;
    let mut current_neg_sum = Decimal::ZERO;

    // Sum the first `period` flows (from index 1 to period)
    for i in 1..=period {
        if i < len {
            current_pos_sum += pos_flows[i];
            current_neg_sum += neg_flows[i];
        }
    }

    if period < len {
        let mfi = if current_neg_sum.is_zero() {
            if current_pos_sum.is_zero() {
                Decimal::new(50, 0)
            } else {
                hundred
            }
        } else {
            let mfr = current_pos_sum / current_neg_sum;
            hundred - (hundred / (Decimal::ONE + mfr))
        };
        mfi_values[period] = mfi.to_f64();
    }

    for i in (period + 1)..len {
        current_pos_sum += pos_flows[i];
        current_neg_sum += neg_flows[i];

        current_pos_sum -= pos_flows[i - period];
        current_neg_sum -= neg_flows[i - period];

        let mfi = if current_neg_sum.is_zero() {
            if current_pos_sum.is_zero() {
                Decimal::new(50, 0)
            } else {
                hundred
            }
        } else {
            let mfr = current_pos_sum / current_neg_sum;
            hundred - (hundred / (Decimal::ONE + mfr))
        };
        mfi_values[i] = mfi.to_f64();
    }

    Ok(Series::new("mfi", mfi_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 12.0, 11.0, 13.0, 12.0],
            "low" => &[8.0, 10.0, 9.0, 11.0, 10.0],
            "close" => &[9.0, 11.0, 10.0, 12.0, 11.0],
            "volume" => &[100.0, 150.0, 120.0, 200.0, 150.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 5);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).unwrap();
        assert!(
            (val2 - 57.8947).abs() < 1e-3,
            "Expected ~57.89, got {}",
            val2
        );

        let val3 = out.get(3).unwrap();
        assert!(
            (val3 - 66.6667).abs() < 1e-3,
            "Expected ~66.67, got {}",
            val3
        );

        let val4 = out.get(4).unwrap();
        assert!(
            (val4 - 59.259).abs() < 1e-3,
            "Expected ~59.26, got {}",
            val4
        );

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_short = df!(
            "high" => &[10.0, 11.0],
            "low" => &[9.0, 10.0],
            "close" => &[9.5, 10.5],
            "volume" => &[100.0, 100.0]
        )?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.f64()?;
        assert_eq!(out.len(), 2);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        // Zero period
        let res_zero = calculate(&df_short, 0);
        assert!(res_zero.is_err());

        // Flat data
        let df_flat = df!(
            "high" => &[10.0, 10.0, 10.0],
            "low" => &[10.0, 10.0, 10.0],
            "close" => &[10.0, 10.0, 10.0],
            "volume" => &[100.0, 100.0, 100.0]
        )?;
        let res_flat = calculate(&df_flat, 2)?;
        let out_flat = res_flat.f64()?;
        assert_eq!(out_flat.get(2), Some(50.0));

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let len = 100;
        let high: Vec<f64> = (0..len)
            .map(|i| 105.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let low: Vec<f64> = (0..len)
            .map(|i| 95.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let close: Vec<f64> = (0..len)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let volume: Vec<f64> = (0..len)
            .map(|i| 1000.0 + (i as f64 * 0.2).cos() * 500.0)
            .collect();

        let df = df!(
            "high" => high,
            "low" => low,
            "close" => close,
            "volume" => volume
        )?;

        let result = calculate(&df, 14);
        assert!(result.is_ok());
        let s = result?;
        assert_eq!(s.len(), 100);
        assert!(s.f64()?.get(13).is_none());
        assert!(s.f64()?.get(14).is_some());

        let series = s.f64()?;
        for i in 14..len {
            if let Some(v) = series.get(i) {
                assert!(v >= 0.0 && v <= 100.0, "MFI {} out of bounds at {}", v, i);
            }
        }

        Ok(())
    }
}
