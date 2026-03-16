//! Ultimate Oscillator (UO)
//!
//! The Ultimate Oscillator is a momentum oscillator developed by Larry Williams.
//! It uses three different timeframes to reduce the volatility and false signals
//! associated with single-timeframe oscillators.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;

/// Calculate the Ultimate Oscillator
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", and "close" columns
/// * `period1` - Short timeframe period (typically 7)
/// * `period2` - Medium timeframe period (typically 14)
/// * `period3` - Long timeframe period (typically 28)
///
/// # Returns
/// Series with indicator values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::ultimate_oscillator;
///
/// // Create sample DataFrame with "high", "low", "close" columns
/// // let result = ultimate_oscillator::calculate(&df, 7, 14, 28)?;
/// ```
pub fn calculate(
    data: &DataFrame,
    period1: usize,
    period2: usize,
    period3: usize,
) -> Result<Series> {
    // Validate inputs
    if data.is_empty() {
        anyhow::bail!("Data cannot be empty");
    }

    if period1 == 0 || period2 == 0 || period3 == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

    let high = data.column("high").context("Missing 'high' column")?;
    let low = data.column("low").context("Missing 'low' column")?;
    let close = data.column("close").context("Missing 'close' column")?;

    if high.null_count() > 0 || low.null_count() > 0 || close.null_count() > 0 {
        anyhow::bail!("Input columns cannot contain null values");
    }

    let high_f64 = high.f64()?;
    let low_f64 = low.f64()?;
    let close_f64 = close.f64()?;

    let len = high_f64.len();
    let mut result_values = vec![None; len];

    let max_period = period1.max(period2).max(period3);

    if len <= max_period {
        return Ok(Series::new("ultimate_oscillator", result_values));
    }

    // Pre-calculate BP and TR
    let mut bp = vec![Decimal::ZERO; len];
    let mut tr = vec![Decimal::ZERO; len];

    for i in 1..len {
        let h = Decimal::from_f64(high_f64.get(i).unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
        let l = Decimal::from_f64(low_f64.get(i).unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
        let c = Decimal::from_f64(close_f64.get(i).unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
        let pc = Decimal::from_f64(close_f64.get(i - 1).unwrap_or(0.0)).unwrap_or(Decimal::ZERO);

        let min_l_pc = l.min(pc);
        let max_h_pc = h.max(pc);

        bp[i] = c - min_l_pc;
        tr[i] = max_h_pc - min_l_pc;
    }

    // O(N) sliding window sum helper
    let get_sums = |period: usize| -> (Vec<Decimal>, Vec<Decimal>) {
        let mut bp_sum = vec![Decimal::ZERO; len];
        let mut tr_sum = vec![Decimal::ZERO; len];

        if len <= period {
            return (bp_sum, tr_sum);
        }

        let mut current_bp_sum = Decimal::ZERO;
        let mut current_tr_sum = Decimal::ZERO;

        for i in 1..=period {
            current_bp_sum += bp[i];
            current_tr_sum += tr[i];
        }

        bp_sum[period] = current_bp_sum;
        tr_sum[period] = current_tr_sum;

        for i in (period + 1)..len {
            current_bp_sum += bp[i] - bp[i - period];
            current_tr_sum += tr[i] - tr[i - period];
            bp_sum[i] = current_bp_sum;
            tr_sum[i] = current_tr_sum;
        }

        (bp_sum, tr_sum)
    };

    let (bp_sum1, tr_sum1) = get_sums(period1);
    let (bp_sum2, tr_sum2) = get_sums(period2);
    let (bp_sum3, tr_sum3) = get_sums(period3);

    let four = Decimal::from(4);
    let two = Decimal::from(2);
    let one = Decimal::from(1);
    let hundred = Decimal::from(100);
    let weight_sum = four + two + one; // 7

    for i in max_period..len {
        let tr1 = tr_sum1[i];
        let tr2 = tr_sum2[i];
        let tr3 = tr_sum3[i];

        if tr1.is_zero() || tr2.is_zero() || tr3.is_zero() {
            continue;
        }

        let avg1 = bp_sum1[i] / tr1;
        let avg2 = bp_sum2[i] / tr2;
        let avg3 = bp_sum3[i] / tr3;

        let uo = hundred * ((four * avg1) + (two * avg2) + (one * avg3)) / weight_sum;
        result_values[i] = Some(uo.to_f64().unwrap_or(0.0));
    }

    Ok(Series::new("ultimate_oscillator", result_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 12.0, 15.0, 14.0, 13.0, 16.0, 18.0, 17.0, 15.0, 14.0],
            "low" => &[8.0, 9.0, 11.0, 10.0, 9.0, 12.0, 14.0, 13.0, 11.0, 10.0],
            "close" => &[9.0, 11.0, 14.0, 12.0, 10.0, 15.0, 17.0, 14.0, 12.0, 11.0]
        )?;

        let result = calculate(&df, 2, 3, 4)?;
        assert_eq!(result.len(), 10);

        let vals = result.f64()?;
        let uo_val = vals.get(4).context("Expected value at index 4 to be Some")?;

        let expected = 100.0 * (4.0 * (3.0 / 8.0) + 2.0 * (6.0 / 12.0) + 1.0 * (8.0 / 15.0)) / 7.0;

        assert!((uo_val - expected).abs() < 1e-10);
        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let empty_df = DataFrame::default();
        let res = calculate(&empty_df, 7, 14, 28);
        assert!(res.is_err(), "Expected error on empty data");

        let df = df!("close" => &[1.0, 2.0])?;
        let res = calculate(&df, 7, 14, 28);
        assert!(res.is_err(), "Expected error on missing columns");

        let df2 = df!(
            "high" => &[10.0, 11.0],
            "low" => &[9.0, 10.0],
            "close" => &[9.5, 10.5]
        )?;
        let res2 = calculate(&df2, 7, 14, 28)?;
        assert_eq!(res2.len(), 2);
        assert!(
            res2.f64()?.get(1).is_none(),
            "Expected None for insufficient data"
        );

        let high_series = Series::new("high", &[Some(10.0), None::<f64>]);
        let low_series = Series::new("low", &[10.0, 10.0]);
        let close_series = Series::new("close", &[10.0, 10.0]);
        let df_nulls = DataFrame::new(vec![high_series, low_series, close_series])?;

        let res_nulls = calculate(&df_nulls, 7, 14, 28);
        assert!(res_nulls.is_err(), "Expected error for null values");

        let df3 = df!(
            "high" => &[10.0, 11.0],
            "low" => &[9.0, 10.0],
            "close" => &[9.5, 10.5]
        )?;
        let res3 = calculate(&df3, 0, 14, 28);
        assert!(res3.is_err(), "Expected error for zero period");

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let n = 100;
        let mut high = Vec::with_capacity(n);
        let mut low = Vec::with_capacity(n);
        let mut close = Vec::with_capacity(n);

        let mut val = 100.0;
        for i in 0..n {
            high.push(val + 2.0);
            low.push(val - 2.0);
            close.push(val + (i as f64 % 3.0) - 1.0);
            val += 0.1;
        }

        let df = df!(
            "high" => &high,
            "low" => &low,
            "close" => &close
        )?;

        let result = calculate(&df, 7, 14, 28)?;
        let vals = result.f64()?;

        assert_eq!(vals.len(), n);
        assert!(vals.get(27).is_none());
        assert!(vals.get(28).is_some());

        let uo_val = vals.get(99).context("Expected value at index 99 to be Some")?;
        assert!(
            (0.0..=100.0).contains(&uo_val),
            "UO should be between 0 and 100"
        );

        Ok(())
    }
}
