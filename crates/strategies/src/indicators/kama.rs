//! Kaufman's Adaptive Moving Average (KAMA)
//!
//! KAMA is an adaptive moving average that adjusts its sensitivity based on market volatility.
//! It responds quickly to price changes during strong trends and remains flat during choppy or sideways markets.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Kaufman's Adaptive Moving Average (KAMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period for the Efficiency Ratio (ER) (typically 10)
/// * `fast_ema_period` - The fast EMA constant period (typically 2)
/// * `slow_ema_period` - The slow EMA constant period (typically 30)
///
/// # Returns
/// Series with KAMA values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use rust_decimal::Decimal;
/// use strategies::indicators::kama;
///
/// let df = df!(
///     "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
/// ).unwrap_or_default();
/// // Calculate with period=3, fast_ema=2, slow_ema=30 for demonstration
/// let result = kama::calculate(&df, 3, 2, 30).unwrap_or_default();
/// ```
pub fn calculate(
    data: &DataFrame,
    period: usize,
    fast_ema_period: usize,
    slow_ema_period: usize,
) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 || fast_ema_period == 0 || slow_ema_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    // Fastest EMA constant (Fast SC)
    let fast_sc = Decimal::from_f64(2.0 / (fast_ema_period as f64 + 1.0))
        .context("Invalid fast EMA period")?;
    // Slowest EMA constant (Slow SC)
    let slow_sc = Decimal::from_f64(2.0 / (slow_ema_period as f64 + 1.0))
        .context("Invalid slow EMA period")?;

    let mut kama_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut prev_kama: Option<Decimal> = None;

    // Use a rolling window to store historical close values to calculate change and volatility
    let mut window: std::collections::VecDeque<Decimal> =
        std::collections::VecDeque::with_capacity(period + 1);

    for i in 0..close.len() {
        let val_opt = close.get(i);

        match val_opt {
            Some(val) => {
                if let Some(d) = Decimal::from_f64_retain(val) {
                    window.push_back(d);

                    if window.len() > period + 1 {
                        window.pop_front();
                    }

                    if window.len() == period + 1 {
                        // Calculate Efficiency Ratio (ER)
                        // Change = absolute change in price over the period
                        // Volatility = sum of absolute changes in price bar-to-bar over the period

                        let first_val = window[0];
                        let last_val = window[period];

                        // Absolute change over the whole period
                        let mut change = last_val - first_val;
                        if change < Decimal::ZERO {
                            change = -change;
                        }

                        // Sum of absolute bar-to-bar changes
                        let mut volatility = Decimal::ZERO;
                        for j in 1..=period {
                            let mut diff = window[j] - window[j - 1];
                            if diff < Decimal::ZERO {
                                diff = -diff;
                            }
                            volatility += diff;
                        }

                        let er = if volatility == Decimal::ZERO {
                            Decimal::ZERO
                        } else {
                            change / volatility
                        };

                        // Calculate Smoothing Constant (SC)
                        // SC = [ER x (fastest SC - slowest SC) + slowest SC]^2
                        let sc_inner = er * (fast_sc - slow_sc) + slow_sc;
                        let sc = sc_inner * sc_inner;

                        // Calculate KAMA
                        // KAMA = Current KAMA = Prior KAMA + SC x (Price - Prior KAMA)
                        if let Some(prev) = prev_kama {
                            let kama = prev + sc * (d - prev);
                            kama_values.push(kama.to_f64());
                            prev_kama = Some(kama);
                        } else {
                            // Seed KAMA with the first calculated price (or simple moving average)
                            // Standard practice often uses the simple moving average of the initial period as the first KAMA
                            let mut sum = Decimal::ZERO;
                            for val in window.iter().take(period + 1).skip(1) {
                                sum += val;
                            }
                            let period_dec = Decimal::from_usize(period)
                                .context("Invalid period for Decimal conversion")?;
                            let seed = sum / period_dec;

                            kama_values.push(seed.to_f64());
                            prev_kama = Some(seed);
                        }
                    } else {
                        // Not enough data to compute ER and KAMA
                        kama_values.push(None);
                    }
                } else {
                    // Invalid f64 value (e.g., NaN)
                    kama_values.push(None);
                    window.clear();
                    prev_kama = None;
                }
            }
            None => {
                // Missing data point
                kama_values.push(None);
                window.clear();
                prev_kama = None;
            }
        }
    }

    let s = Series::new("kama", kama_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            // 6 values to test period 3
            "close" => &[10.0, 11.0, 10.5, 12.0, 11.5, 13.0]
        )?;

        // period=3, fast=2, slow=30
        let result = calculate(&df, 3, 2, 30)?;
        let out = result.f64()?;

        // Index 0: 10.0 (window: 10)
        // Index 1: 11.0 (window: 10, 11)
        // Index 2: 10.5 (window: 10, 11, 10.5)
        // Index 3: 12.0 (window: 10, 11, 10.5, 12.0) - size=4 (period+1). Seed KAMA calculated.
        // Seed = SMA of [11.0, 10.5, 12.0] = (11+10.5+12)/3 = 33.5/3 = 11.166...
        // Index 4: 11.5 (window: 11, 10.5, 12, 11.5) -> ER calculated -> KAMA updated
        // Index 5: 13.0 (window: 10.5, 12, 11.5, 13) -> ER calculated -> KAMA updated

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());

        // Test the first populated value (the seed)
        let seed_val = out.get(3).unwrap_or(0.0);
        assert!((seed_val - 11.166666666666666).abs() < 1e-10);

        // Next values should be populated
        assert!(out.get(4).is_some());
        assert!(out.get(5).is_some());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 10, 2, 30);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 10, 2, 30)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        // Zero periods
        let df_valid = df!("close" => &[10.0, 11.0, 12.0])?;
        assert!(calculate(&df_valid, 0, 2, 30).is_err());
        assert!(calculate(&df_valid, 10, 0, 30).is_err());
        assert!(calculate(&df_valid, 10, 2, 0).is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut closes = Vec::new();
        // Generate a 100-period uptrend
        for i in 0..100 {
            closes.push(100.0 + (i as f64) * 0.5);
        }

        let df = df!("close" => &closes)?;
        let result = calculate(&df, 10, 2, 30)?;

        assert_eq!(result.len(), 100);
        let out = result.f64()?;
        assert!(out.get(9).is_none());
        assert!(out.get(10).is_some()); // Seed value (period+1 points)
        assert!(out.get(99).is_some());

        // In a perfect uptrend, ER is 1.0 (Change = Volatility).
        // SC = [1.0 * (fast_sc - slow_sc) + slow_sc]^2 = fast_sc^2
        // KAMA should track the trend very closely with the fast EMA constant.
        let final_kama = out.get(99).unwrap_or(0.0);
        assert!(final_kama > 100.0); // It should definitely be greater than the start price

        Ok(())
    }
}
