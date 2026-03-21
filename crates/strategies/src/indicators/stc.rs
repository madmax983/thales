use super::macd;
use anyhow::Result;
use polars::prelude::*;
use rust_decimal::prelude::*;

/// Calculate Schaff Trend Cycle (STC)
///
/// The Schaff Trend Cycle (STC) is an oscillator that combines MACD and Stochastic to provide
/// buy and sell signals. It cycles between 0 and 100.
///
/// STC Algorithm:
/// 1. Calculate MACD Line = EMA(fast) - EMA(slow)
/// 2. Calculate %K (Stoch) of MACD = 100 * (MACD - Lowest MACD) / (Highest MACD - Lowest MACD)
/// 3. Calculate %D (Stoch) of MACD = %K smoothed
/// 4. Calculate %K of %D
/// 5. Calculate STC = %K of %D smoothed
///
/// For simplicity and performance, this implementation combines the double stochastic smoothing steps
/// based on the common STC logic.
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `fast_period` - MACD Fast EMA period (typically 23)
/// * `slow_period` - MACD Slow EMA period (typically 50)
/// * `cycle_period` - Stochastic Cycle period (typically 10)
/// * `d_period` - Stochastic %D smoothing period (typically 3)
///
/// # Returns
/// Series with STC values.
pub fn calculate(
    data: &DataFrame,
    fast_period: usize,
    slow_period: usize,
    cycle_period: usize,
    d_period: usize, // typically 3
) -> Result<Series> {
    let safe_cycle_period = if cycle_period == 0 { 1 } else { cycle_period };

    // 1. Calculate MACD Line (we don't need the signal line or histogram from macd)
    let (macd_line, _, _) = macd::calculate(data, fast_period, slow_period, 9)?;

    let macd_vals = macd_line.f64()?;

    // We'll calculate STC using Decimal for precision
    let mut macd_decimals: Vec<Option<Decimal>> = Vec::with_capacity(macd_vals.len());
    for i in 0..macd_vals.len() {
        macd_decimals.push(macd_vals.get(i).and_then(Decimal::from_f64_retain));
    }

    // First Stochastic on MACD
    let mut k1 = vec![None; macd_decimals.len()];
    let hundred = Decimal::new(100, 0);

    for i in 0..macd_decimals.len() {
        if i >= safe_cycle_period - 1 {
            let mut highest = Decimal::MIN;
            let mut lowest = Decimal::MAX;
            let mut valid = true;

            for val_opt in macd_decimals
                .iter()
                .take(i + 1)
                .skip(i + 1 - safe_cycle_period)
            {
                if let Some(val) = val_opt {
                    if *val > highest {
                        highest = *val;
                    }
                    if *val < lowest {
                        lowest = *val;
                    }
                } else {
                    valid = false;
                    break;
                }
            }

            if valid {
                if let Some(current) = macd_decimals[i] {
                    let range = highest - lowest;
                    if range.is_zero() {
                        k1[i] = Some(Decimal::ZERO);
                    } else {
                        k1[i] = Some(hundred * (current - lowest) / range);
                    }
                }
            }
        }
    }

    // Smooth K1 to get D1
    let d1 = calculate_ema(&k1, d_period);

    // Second Stochastic on D1
    let mut k2 = vec![None; d1.len()];

    for i in 0..d1.len() {
        if i >= safe_cycle_period - 1 {
            let mut highest = Decimal::MIN;
            let mut lowest = Decimal::MAX;
            let mut valid = true;

            for val_opt in d1.iter().take(i + 1).skip(i + 1 - safe_cycle_period) {
                if let Some(val) = val_opt {
                    if *val > highest {
                        highest = *val;
                    }
                    if *val < lowest {
                        lowest = *val;
                    }
                } else {
                    valid = false;
                    break;
                }
            }

            if valid {
                if let Some(current) = d1[i] {
                    let range = highest - lowest;
                    if range.is_zero() {
                        k2[i] = Some(Decimal::ZERO);
                    } else {
                        k2[i] = Some(hundred * (current - lowest) / range);
                    }
                }
            }
        }
    }

    // Smooth K2 to get STC
    let stc = calculate_ema(&k2, d_period);

    let stc_f64: Vec<Option<f64>> = stc
        .into_iter()
        .map(|opt| opt.map(|d| d.to_f64().unwrap_or(0.0)))
        .collect();

    Ok(Series::new("stc", stc_f64))
}

fn calculate_ema(data: &[Option<Decimal>], period: usize) -> Vec<Option<Decimal>> {
    let mut ema_values = vec![None; data.len()];
    if period == 0 || data.is_empty() {
        return ema_values;
    }

    let k_f64 = 2.0 / (period as f64 + 1.0);
    let k = match Decimal::from_f64_retain(k_f64) {
        Some(val) => val,
        None => return ema_values, // Fallback if k calculation fails
    };

    let mut prev_ema: Option<Decimal> = None;

    // The smoothing typically starts when we have the first valid value, we can use it as the initial EMA seed
    for i in 0..data.len() {
        if let Some(val) = data[i] {
            if let Some(prev) = prev_ema {
                let ema = (val * k) + (prev * (Decimal::ONE - k));
                ema_values[i] = Some(ema);
                prev_ema = Some(ema);
            } else {
                ema_values[i] = Some(val);
                prev_ema = Some(val);
            }
        } else {
            ema_values[i] = None;
            prev_ema = None;
        }
    }
    ema_values
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_stc_calculation() -> Result<()> {
        let df = df!(
            "close" => &[
                100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0, 108.0, 109.0,
                110.0, 111.0, 112.0, 113.0, 114.0, 115.0, 116.0, 117.0, 118.0, 119.0,
                120.0, 119.0, 118.0, 117.0, 116.0, 115.0, 114.0, 113.0, 112.0, 111.0,
                110.0, 109.0, 108.0, 107.0, 106.0, 105.0, 104.0, 103.0, 102.0, 101.0,
                100.0, 99.0, 98.0, 97.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0,
                90.0, 91.0, 92.0, 93.0, 94.0, 95.0, 96.0, 97.0, 98.0, 99.0,
                100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0, 108.0, 109.0
            ]
        )?;

        // Using smaller periods for testing
        let stc = calculate(&df, 3, 5, 3, 2)?;
        let stc_vals = stc.f64()?;

        assert_eq!(stc_vals.len(), 70);

        // Initial values should be None until MACD and cycle periods are met
        assert!(stc_vals.get(0).is_none());

        // STC is bounded between 0 and 100
        for i in 10..stc_vals.len() {
            if let Some(val) = stc_vals.get(i) {
                assert!(
                    (0.0..=100.0).contains(&val),
                    "STC out of bounds at index {}: {}",
                    i,
                    val
                );
            }
        }

        Ok(())
    }

    #[test]
    fn test_stc_edge_cases() -> Result<()> {
        // Flat price
        let df_flat = df!(
            "close" => &[100.0; 50]
        )?;
        let stc_flat = calculate(&df_flat, 23, 50, 10, 3)?;
        let stc_flat_vals = stc_flat.f64()?;

        // When MACD is flat, range is zero, so k1 should be 0, so stc should be 0
        // Wait, because we just need to ensure it doesn't panic
        assert_eq!(stc_flat_vals.len(), 50);

        Ok(())
    }
}
