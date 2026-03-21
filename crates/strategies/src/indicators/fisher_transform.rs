//! Fisher Transform
//!
//! Calculates the Fisher Transform, a technical indicator developed by John F. Ehlers
//! that converts price data into a Gaussian normal distribution.
//!
//! The indicator helps identify potential turning points and trends in asset prices.

use anyhow::Result;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Fisher Transform
///
/// # Arguments
/// * `data` - DataFrame with "high" and "low" columns (or "close" if "high"/"low" are not present)
/// * `period` - Lookback period (standard is 9)
///
/// # Returns
/// Series with Fisher Transform values. The first `period - 1` values will be null.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let len = data.height();

    // We ideally want high/low prices, but fallback to close if they aren't available
    let (high, low) = if data.column("high").is_ok() && data.column("low").is_ok() {
        let h = data.column("high")?.f64()?;
        let l = data.column("low")?.f64()?;
        (h.clone(), l.clone())
    } else if let Ok(close_col) = data.column("close") {
        let c = close_col.f64()?;
        (c.clone(), c.clone())
    } else {
        anyhow::bail!("DataFrame must contain either 'high'/'low' or 'close' columns");
    };

    let mut fisher_values: Vec<Option<f64>> = vec![None; len];

    if len < period {
        return Ok(Series::new("fisher_transform", fisher_values));
    }

    let mut value1 = Decimal::ZERO;
    let mut prev_fisher = Decimal::ZERO;

    // Convert high/low series to vectors to iterate over windows
    let h_vec: Vec<Option<f64>> = high.into_iter().collect();
    let l_vec: Vec<Option<f64>> = low.into_iter().collect();

    for i in (period - 1)..len {
        let mut highest_high = Decimal::MIN;
        let mut lowest_low = Decimal::MAX;
        let mut valid_window = true;

        for j in 0..period {
            let h_opt = h_vec[i - j];
            let l_opt = l_vec[i - j];

            match (h_opt, l_opt) {
                (Some(h_val), Some(l_val)) => {
                    let h_dec = Decimal::from_f64_retain(h_val).unwrap_or(Decimal::ZERO);
                    let l_dec = Decimal::from_f64_retain(l_val).unwrap_or(Decimal::ZERO);

                    if h_dec > highest_high { highest_high = h_dec; }
                    if l_dec < lowest_low { lowest_low = l_dec; }
                },
                _ => {
                    valid_window = false;
                    break;
                }
            }
        }

        if !valid_window {
            fisher_values[i] = None;
            continue;
        }

        // Calculate median price for current period (typically (High + Low) / 2)
        // Here we just use the current period's median
        let curr_h = Decimal::from_f64_retain(h_vec[i].unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
        let curr_l = Decimal::from_f64_retain(l_vec[i].unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
        let price = (curr_h + curr_l) / Decimal::TWO;

        let range = highest_high - lowest_low;

        // Normalize the price to a value between -1 and 1
        let x = if range > Decimal::ZERO {
            let p_norm = (price - lowest_low) / range; // 0 to 1
            p_norm * Decimal::TWO - Decimal::ONE // -1 to 1
        } else {
            Decimal::ZERO
        };

        // Smooth value1
        let x_smoothed = Decimal::new(66, 2) * x + Decimal::new(33, 2) * value1;
        value1 = x_smoothed;

        // Truncate value1 to avoid infinities
        let limit = Decimal::new(999, 3); // 0.999
        let neg_limit = limit * Decimal::NEGATIVE_ONE; // -0.999

        if value1 > limit {
            value1 = limit;
        } else if value1 < neg_limit {
            value1 = neg_limit;
        }

        // Calculate Fisher Transform
        // Formula: 0.5 * ln((1 + X) / (1 - X))
        let one = Decimal::ONE;
        let num = one + value1;
        let den = one - value1;

        let mut current_fisher = Decimal::ZERO;
        if den > Decimal::ZERO {
            let ratio = num / den;
            let ratio_f64 = ratio.to_f64().unwrap_or(1.0);

            if ratio_f64 > 0.0 {
                let ln_val = ratio_f64.ln();
                let ln_dec = Decimal::from_f64_retain(ln_val).unwrap_or(Decimal::ZERO);

                current_fisher = Decimal::new(5, 1) * ln_dec + Decimal::new(5, 1) * prev_fisher;
            }
        }

        fisher_values[i] = Some(current_fisher.to_f64().unwrap_or(0.0));
        prev_fisher = current_fisher;
    }

    Ok(Series::new("fisher_transform", fisher_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_fisher_transform() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 11.0, 12.0, 11.0, 10.0, 9.0, 8.0, 9.0, 10.0, 11.0],
            "low" => &[9.0, 10.0, 11.0, 10.0, 9.0, 8.0, 7.0, 8.0, 9.0, 10.0]
        )?;

        let result = calculate(&df, 4)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 10);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());
        assert!(out.get(3).is_some());

        Ok(())
    }
}
