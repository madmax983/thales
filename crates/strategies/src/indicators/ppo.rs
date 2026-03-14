//! Percentage Price Oscillator (PPO) - A momentum oscillator that measures the difference between two moving averages as a percentage of the larger moving average.

use super::ema;
use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;

/// Calculate Percentage Price Oscillator (PPO)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `fast_period` - Fast EMA period (default 12)
/// * `slow_period` - Slow EMA period (default 26)
/// * `signal_period` - Signal Line EMA period (default 9)
///
/// # Returns
/// Tuple of (PPO Line, Signal Line, Histogram) Series
///
/// # Example
/// ```rust
/// use anyhow::Result;
/// use polars::prelude::*;
/// use strategies::indicators::ppo;
///
/// fn example() -> Result<()> {
///     let df = df!(
///         "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0]
///     )?;
///     let (ppo_line, signal_line, hist) = ppo::calculate(&df, 2, 4, 2)?;
///     Ok(())
/// }
/// ```
pub fn calculate(
    data: &DataFrame,
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> Result<(Series, Series, Series)> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    if fast_period >= slow_period {
        anyhow::bail!("fast_period must be less than slow_period");
    }

    // Calculate Fast and Slow EMAs
    let fast_ema_series =
        ema::calculate(data, fast_period).context("Failed to calculate Fast EMA")?;
    let slow_ema_series =
        ema::calculate(data, slow_period).context("Failed to calculate Slow EMA")?;

    let fast_ema = fast_ema_series.f64()?;
    let slow_ema = slow_ema_series.f64()?;

    let len = data.height();
    let mut ppo_values: Vec<Option<f64>> = Vec::with_capacity(len);
    let hundred = Decimal::new(100, 0);

    for i in 0..len {
        let fast_val = fast_ema.get(i);
        let slow_val = slow_ema.get(i);

        match (fast_val, slow_val) {
            (Some(f), Some(s)) if !s.is_nan() && s != 0.0 && !f.is_nan() => {
                let f_dec = Decimal::from_f64_retain(f).unwrap_or(Decimal::ZERO);
                let s_dec = Decimal::from_f64_retain(s).unwrap_or(Decimal::ZERO);

                if !s_dec.is_zero() {
                    let ppo = ((f_dec - s_dec) / s_dec) * hundred;
                    ppo_values.push(ppo.to_f64());
                } else {
                    ppo_values.push(None);
                }
            }
            _ => ppo_values.push(None),
        }
    }

    let ppo_line = Series::new("ppo_line", ppo_values);

    // Calculate Signal Line = EMA(PPO Line, signal_period)
    let temp_df = DataFrame::new(vec![ppo_line.clone()])?;
    let mut temp_df = temp_df;
    temp_df.rename("ppo_line", "close")?;

    let mut signal_line =
        ema::calculate(&temp_df, signal_period).context("Failed to calculate Signal Line")?;

    signal_line.rename("ppo_signal");

    // Calculate Histogram = PPO Line - Signal Line iteratively with Decimal
    let mut hist_values: Vec<Option<f64>> = Vec::with_capacity(len);
    let signal_vals = signal_line.f64()?;
    let ppo_vals = ppo_line.f64()?;

    for i in 0..len {
        let p_val = ppo_vals.get(i);
        let s_val = signal_vals.get(i);

        match (p_val, s_val) {
            (Some(p), Some(s)) if !p.is_nan() && !s.is_nan() => {
                let p_dec = Decimal::from_f64_retain(p).unwrap_or(Decimal::ZERO);
                let s_dec = Decimal::from_f64_retain(s).unwrap_or(Decimal::ZERO);

                let hist = p_dec - s_dec;
                hist_values.push(hist.to_f64());
            }
            _ => hist_values.push(None),
        }
    }

    let histogram = Series::new("ppo_hist", hist_values);

    Ok((ppo_line, signal_line, histogram))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_ppo_calculation() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 12.0, 14.0, 16.0, 18.0, 20.0]
        )?;

        // Values trace (refer to MACD tests):
        // Fast EMA (2): [None, 11.0, 13.0, 15.0, 17.0, 19.0]
        // Slow EMA (4): [None, None, None, 13.0, 15.0, 17.0]
        // PPO Line = ((Fast - Slow) / Slow) * 100
        // 3: ((15.0 - 13.0) / 13.0) * 100 = (2.0 / 13.0) * 100 = 15.38461538
        // 4: ((17.0 - 15.0) / 15.0) * 100 = (2.0 / 15.0) * 100 = 13.33333333
        // 5: ((19.0 - 17.0) / 17.0) * 100 = (2.0 / 17.0) * 100 = 11.76470588

        // Signal Line (2) of PPO Line
        // 3: None
        // 4: (15.3846 + 13.3333) / 2 = 14.358974355 (SMA seed)
        // 5: EMA(11.7647) = (11.7647 * 2/3) + (14.3589 * 1/3) = 7.84313 + 4.7863 = 12.62943

        let (ppo, signal, _hist) = calculate(&df, 2, 4, 2)?;

        let ppo_vals = ppo.f64()?;
        let signal_vals = signal.f64()?;

        // Verify PPO Line
        assert!(ppo_vals.get(0).is_none());
        assert!(ppo_vals.get(1).is_none());
        assert!(ppo_vals.get(2).is_none());

        let val3 = ppo_vals.get(3).context("Missing val3")?;
        assert!((val3 - 15.38461538).abs() < 1e-4);

        let val4 = ppo_vals.get(4).context("Missing val4")?;
        assert!((val4 - 13.33333333).abs() < 1e-4);

        let val5 = ppo_vals.get(5).context("Missing val5")?;
        assert!((val5 - 11.76470588).abs() < 1e-4);

        // Verify Signal Line
        assert!(signal_vals.get(3).is_none());

        let sval4 = signal_vals.get(4).context("Missing sval4")?;
        assert!((sval4 - 14.35897435).abs() < 1e-4);

        let sval5 = signal_vals.get(5).context("Missing sval5")?;
        assert!((sval5 - 12.62948624).abs() < 1e-4);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty, 12, 26, 9).is_err());

        let df_short = df!("close" => &[10.0])?;
        let (ppo, _, _) = calculate(&df_short, 2, 4, 2)?;
        assert!(ppo.f64()?.get(0).is_none());

        // Invalid params
        assert!(calculate(&df_short, 26, 12, 9).is_err()); // fast >= slow

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[100.0, 102.0, 104.0, 103.0, 105.0, 108.0, 110.0, 112.0, 115.0, 113.0]
        )?;

        let (ppo, signal, hist) = calculate(&df, 2, 4, 2)?;
        assert_eq!(ppo.len(), 10);
        assert_eq!(signal.len(), 10);
        assert_eq!(hist.len(), 10);

        Ok(())
    }
}
