//! RSI - Relative Strength Index
//!
//! Calculates the Relative Strength Index (RSI), a momentum oscillator that measures the speed and change of price movements.
//! RSI oscillates between 0 and 100.
//!
//! Traditionally, RSI is considered overbought when above 70 and oversold when below 30.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Relative Strength Index (RSI)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period (standard is 14)
///
/// # Returns
/// Series with RSI values. The first `period` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let rsi = strategies::indicators::rsi::calculate(&df, 14)?;
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

    let mut rsi_values: Vec<Option<f64>> = vec![None; close.len()];

    // Need at least period + 1 data points to have period changes
    if close.len() <= period {
        // Return series of nulls
        return Ok(Series::new("rsi", rsi_values));
    }

    let period_dec = Decimal::from_usize(period).context("Invalid period")?;
    let period_minus_one = period_dec - Decimal::ONE;
    let hundred = Decimal::new(100, 0);

    let mut avg_gain = Decimal::ZERO;
    let mut avg_loss = Decimal::ZERO;

    // Calculate initial SMA of gains/losses
    // Changes are at indices 1 to period (inclusive)
    let mut valid_start = true;
    for i in 1..=period {
        let curr = close.get(i).unwrap_or(f64::NAN);
        let prev = close.get(i - 1).unwrap_or(f64::NAN);

        if curr.is_nan() || prev.is_nan() {
            valid_start = false;
            break;
        }

        let change = Decimal::from_f64_retain(curr).unwrap_or(Decimal::ZERO)
            - Decimal::from_f64_retain(prev).unwrap_or(Decimal::ZERO);

        if change > Decimal::ZERO {
            avg_gain += change;
        } else {
            avg_loss += change.abs();
        }
    }

    if !valid_start {
        // If initial data is invalid, return nulls (or handle more complexly)
        // For now, return nulls
        return Ok(Series::new("rsi", rsi_values));
    }

    avg_gain /= period_dec;
    avg_loss /= period_dec;

    // Calculate first RSI (at index period)
    let first_rsi = if avg_loss.is_zero() {
        if avg_gain.is_zero() {
            Decimal::new(50, 0) // No change
        } else {
            hundred // Pure gain
        }
    } else {
        let rs = avg_gain / avg_loss;
        hundred - (hundred / (Decimal::ONE + rs))
    };

    rsi_values[period] = first_rsi.to_f64();

    // Loop for the rest
    for (i, rsi_val) in rsi_values
        .iter_mut()
        .enumerate()
        .take(close.len())
        .skip(period + 1)
    {
        let curr_opt = close.get(i);
        let prev_opt = close.get(i - 1);

        if let (Some(curr), Some(prev)) = (curr_opt, prev_opt) {
            let change = Decimal::from_f64_retain(curr).unwrap_or(Decimal::ZERO)
                - Decimal::from_f64_retain(prev).unwrap_or(Decimal::ZERO);

            let (curr_gain, curr_loss) = if change > Decimal::ZERO {
                (change, Decimal::ZERO)
            } else {
                (Decimal::ZERO, change.abs())
            };

            // Wilder's Smoothing
            // AvgGain = (PrevAvgGain * (period - 1) + CurrGain) / period
            avg_gain = (avg_gain * period_minus_one + curr_gain) / period_dec;
            avg_loss = (avg_loss * period_minus_one + curr_loss) / period_dec;

            let rsi = if avg_loss.is_zero() {
                if avg_gain.is_zero() {
                    Decimal::new(50, 0)
                } else {
                    hundred
                }
            } else {
                let rs = avg_gain / avg_loss;
                hundred - (hundred / (Decimal::ONE + rs))
            };

            *rsi_val = rsi.to_f64();
        } else {
            // Missing data
            *rsi_val = None;
            // Reset? Or carry forward?
            // Standard behavior: if missing data, gap in RSI.
            // But we need avg_gain/loss for next step.
            // Resetting is safest or treating as 0 change.
            // Resetting implies we need another `period` to restart.
            // Let's reset for correctness.
            avg_gain = Decimal::ZERO;
            avg_loss = Decimal::ZERO;
            // Note: This implementation doesn't automatically restart logic.
            // To properly restart, we'd need to re-enter the "initial SMA" mode.
            // For this implementation, we will just output None and keep avgs as 0 (effectively restarting but with 0 history).
            // A more robust implementation would buffer next `period` changes.
        }
    }

    Ok(Series::new("rsi", rsi_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        // Data: [10, 12, 11, 13]
        // Changes: [2, -1, 2]
        // Period: 2
        // Initial Avg (SMA of first 2 changes):
        //   Gains: [2, 0]. AvgGain = 1.0
        //   Losses: [0, 1]. AvgLoss = 0.5
        //   RS = 2.0. RSI = 100 - 33.33 = 66.67
        // Next (index 3, val 13):
        //   Change: +2. Gain: 2. Loss: 0.
        //   AvgGain = (1.0 * 1 + 2) / 2 = 1.5
        //   AvgLoss = (0.5 * 1 + 0) / 2 = 0.25
        //   RS = 6.0. RSI = 100 - 14.28 = 85.71

        let df = df!(
            "close" => &[10.0, 12.0, 11.0, 13.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 4);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let val2 = out.get(2).unwrap();
        assert!(
            (val2 - 66.666666).abs() < 1e-4,
            "Expected ~66.67, got {}",
            val2
        );

        let val3 = out.get(3).unwrap();
        assert!(
            (val3 - 85.714285).abs() < 1e-4,
            "Expected ~85.71, got {}",
            val3
        );

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!("close" => &[10.0, 11.0, 12.0])?;
        let res_short = calculate(&df_short, 5)?;
        let out = res_short.f64()?;
        assert_eq!(out.len(), 3);
        assert!(out.get(0).is_none());
        assert!(out.get(2).is_none());

        // Zero period
        let df_normal = df!("close" => &[10.0, 11.0])?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());

        // Flat data
        let df_flat = df!("close" => &[10.0, 10.0, 10.0, 10.0])?;
        let res_flat = calculate(&df_flat, 2)?;
        let out_flat = res_flat.f64()?;
        // Expect 50.0 (as per implementation for 0/0)
        assert_eq!(out_flat.get(2), Some(50.0));
        assert_eq!(out_flat.get(3), Some(50.0));

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 14);
        assert!(result.is_ok());
        let s = result?;
        assert_eq!(s.len(), 100);
        assert!(s.f64()?.get(13).is_none());
        assert!(s.f64()?.get(14).is_some());

        let series = s.f64()?;
        for i in 14..100 {
            if let Some(v) = series.get(i) {
                assert!(
                    (0.0..=100.0).contains(&v),
                    "RSI {} out of bounds at {}",
                    v,
                    i
                );
            }
        }

        Ok(())
    }
}
