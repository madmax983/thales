//! Ultimate Oscillator
//!
//! Calculates the Ultimate Oscillator, a technical indicator developed by Larry Williams
//! that measures momentum across three different timeframes to reduce false divergence signals.
//! The oscillator is typically bounded between 0 and 100, where readings below 30 denote oversold conditions
//! and readings above 70 denote overbought conditions.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Ultimate Oscillator
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", and "close" columns
/// * `period1` - Short lookback period (typically 7)
/// * `period2` - Medium lookback period (typically 14)
/// * `period3` - Long lookback period (typically 28)
///
/// # Returns
/// Series with Ultimate Oscillator values. Initial values will be null until enough data is gathered.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let ultimate = strategies::indicators::ultimate_oscillator::calculate(&df, 7, 14, 28)?;
/// ```
pub fn calculate(
    data: &DataFrame,
    period1: usize,
    period2: usize,
    period3: usize,
) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period1 == 0 || period2 == 0 || period3 == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }
    if period1 >= period2 || period2 >= period3 {
        anyhow::bail!("Periods must be strictly increasing: period1 < period2 < period3");
    }

    let high_s = data
        .column("high")
        .context("DataFrame must contain 'high' column")?
        .cast(&DataType::Float64)?;
    let high = high_s.f64().context("High column must be numeric")?;

    let low_s = data
        .column("low")
        .context("DataFrame must contain 'low' column")?
        .cast(&DataType::Float64)?;
    let low = low_s.f64().context("Low column must be numeric")?;

    let close_s = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .cast(&DataType::Float64)?;
    let close = close_s.f64().context("Close column must be numeric")?;

    let len = close.len();
    let mut ultimate_values: Vec<Option<f64>> = vec![None; len];

    if len <= period3 {
        return Ok(Series::new("ultimate_oscillator", ultimate_values));
    }

    let hundred = Decimal::new(100, 0);
    let d4 = Decimal::new(4, 0);
    let d2 = Decimal::new(2, 0);
    let d1 = Decimal::ONE;
    let sum_weights = d4 + d2 + d1;

    // True Range and Buying Pressure
    let mut bp: Vec<Option<Decimal>> = vec![None; len];
    let mut tr: Vec<Option<Decimal>> = vec![None; len];

    for i in 1..len {
        let curr_high = match high.get(i) {
            Some(v) => Decimal::from_f64_retain(v),
            None => None,
        };
        let curr_low = match low.get(i) {
            Some(v) => Decimal::from_f64_retain(v),
            None => None,
        };
        let curr_close = match close.get(i) {
            Some(v) => Decimal::from_f64_retain(v),
            None => None,
        };
        let prev_close = match close.get(i - 1) {
            Some(v) => Decimal::from_f64_retain(v),
            None => None,
        };

        if let (Some(ch), Some(cl), Some(cc), Some(pc)) =
            (curr_high, curr_low, curr_close, prev_close)
        {
            let true_low = cl.min(pc);
            let true_high = ch.max(pc);

            bp[i] = Some(cc - true_low);
            tr[i] = Some(true_high - true_low);
        }
    }

    // Precalculate rolling sums to optimize O(N*K) down to O(N)
    // Actually, since period3 is max 28 usually, O(N*K) is extremely small,
    // but computing rolling sum properly is better.
    // For simplicity and to match other indicators in this crate, we iterate,
    // but we correctly handle None values to avoid corrupting data with $0.00.
    #[allow(clippy::needless_range_loop)]
    for i in period3..len {
        let mut valid = true;
        let mut sum_bp1 = Decimal::ZERO;
        let mut sum_tr1 = Decimal::ZERO;
        let mut sum_bp2 = Decimal::ZERO;
        let mut sum_tr2 = Decimal::ZERO;
        let mut sum_bp3 = Decimal::ZERO;
        let mut sum_tr3 = Decimal::ZERO;

        for j in (i + 1 - period3)..=i {
            if let (Some(bp_val), Some(tr_val)) = (bp[j], tr[j]) {
                sum_bp3 += bp_val;
                sum_tr3 += tr_val;

                if j >= (i + 1 - period2) {
                    sum_bp2 += bp_val;
                    sum_tr2 += tr_val;
                }

                if j >= (i + 1 - period1) {
                    sum_bp1 += bp_val;
                    sum_tr1 += tr_val;
                }
            } else {
                valid = false;
                break;
            }
        }

        if !valid {
            continue; // Leaves as None
        }

        if sum_tr1.is_zero() || sum_tr2.is_zero() || sum_tr3.is_zero() {
            ultimate_values[i] = Some(50.0); // Neutral
            continue;
        }

        let avg1 = sum_bp1 / sum_tr1;
        let avg2 = sum_bp2 / sum_tr2;
        let avg3 = sum_bp3 / sum_tr3;

        let uo = hundred * ((d4 * avg1) + (d2 * avg2) + (d1 * avg3)) / sum_weights;
        ultimate_values[i] = uo.to_f64();
    }

    Ok(Series::new("ultimate_oscillator", ultimate_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0],
            "low" => &[9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0],
            "close" => &[9.5, 10.5, 11.5, 12.5, 13.5, 14.5, 15.5]
        )?;

        // Period3 = 6
        let res = calculate(&df, 2, 4, 6)?;
        let out = res.f64()?;

        assert_eq!(out.len(), 7);
        assert!(out.get(0).is_none());
        assert!(out.get(5).is_none());
        assert!(out.get(6).is_some());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 7, 14, 28);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        let df_invalid_periods =
            df!("high" => &[10.0, 11.0], "low" => &[9.0, 10.0], "close" => &[9.5, 10.5])?;
        let res_invalid = calculate(&df_invalid_periods, 14, 7, 28);
        assert!(res_invalid.is_err());
        assert_eq!(
            res_invalid.unwrap_err().to_string(),
            "Periods must be strictly increasing: period1 < period2 < period3"
        );

        let res_short = calculate(&df_invalid_periods, 2, 4, 6)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let highs: Vec<f64> = (0..50).map(|i| 100.0 + i as f64).collect();
        let lows: Vec<f64> = (0..50).map(|i| 90.0 + i as f64).collect();
        let closes: Vec<f64> = (0..50).map(|i| 95.0 + i as f64).collect();

        let df = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let res = calculate(&df, 7, 14, 28)?;
        let out = res.f64()?;

        assert_eq!(out.len(), 50);
        assert!(out.get(27).is_none());
        assert!(out.get(28).is_some());

        for i in 28..50 {
            if let Some(val) = out.get(i) {
                assert!(
                    (0.0..=100.0).contains(&val),
                    "UO {} out of bounds at {}",
                    val,
                    i
                );
            }
        }

        Ok(())
    }
}
