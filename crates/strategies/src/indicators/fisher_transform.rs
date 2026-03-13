//! Fisher Transform Indicator
//!
//! The Fisher Transform indicator, created by John F. Ehlers, turns price data into a normal
//! distribution to better identify turning points and price extremes.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate the Fisher Transform
///
/// # Arguments
/// * `data` - DataFrame with "high" and "low" columns. "close" can be used instead of HL/2, but HL is standard.
/// * `period` - Lookback period (standard is 9 or 10)
///
/// # Returns
/// A tuple containing:
/// 1. Fisher Transform `Series`
/// 2. Previous Fisher Transform `Series` (often used for crossover signals)
pub fn calculate(data: &DataFrame, period: usize) -> Result<(Series, Series)> {
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

    let len = data.height();
    let mut fisher_vals: Vec<Option<f64>> = vec![None; len];
    let mut fisher_signal_vals: Vec<Option<f64>> = vec![None; len];

    if len < period {
        return Ok((
            Series::new("fisher", fisher_vals),
            Series::new("fisher_signal", fisher_signal_vals),
        ));
    }

    let mut prev_value = Decimal::ZERO;
    let mut prev_fisher = Decimal::ZERO;

    for i in (period - 1)..len {
        let mut min_low = Decimal::MAX;
        let mut max_high = Decimal::MIN;

        // Find min_low and max_high over the period window
        let start_idx = i + 1 - period;
        let mut valid_window = true;

        for j in start_idx..=i {
            if let (Some(h_val), Some(l_val)) = (high.get(j), low.get(j)) {
                if let (Some(h_dec), Some(l_dec)) = (
                    Decimal::from_f64_retain(h_val),
                    Decimal::from_f64_retain(l_val),
                ) {
                    if h_dec > max_high {
                        max_high = h_dec;
                    }
                    if l_dec < min_low {
                        min_low = l_dec;
                    }
                } else {
                    valid_window = false;
                    break;
                }
            } else {
                valid_window = false;
                break;
            }
        }

        if !valid_window || min_low == Decimal::MAX || max_high == Decimal::MIN {
            prev_value = Decimal::ZERO;
            prev_fisher = Decimal::ZERO;
            continue;
        }

        // Current price P is often taken as (High + Low) / 2
        let current_high = high.get(i).unwrap_or(0.0);
        let current_low = low.get(i).unwrap_or(0.0);
        let current_price =
            Decimal::from_f64_retain((current_high + current_low) / 2.0).unwrap_or(Decimal::ZERO);

        let mut value;
        if max_high != min_low {
            let p_norm = (current_price - min_low) / (max_high - min_low);
            let half = Decimal::from_f64_retain(0.5).unwrap_or(Decimal::ZERO);
            let two = Decimal::from_f64_retain(2.0).unwrap_or(Decimal::ZERO);
            let c_066 = Decimal::from_f64_retain(0.66).unwrap_or(Decimal::ZERO);
            let c_067 = Decimal::from_f64_retain(0.67).unwrap_or(Decimal::ZERO);

            let p_adj = p_norm - half;
            value = c_066 * p_adj * two + c_067 * prev_value;
        } else {
            value = Decimal::ZERO;
        }

        // Truncate to avoid inf/-inf in log
        let max_val = Decimal::from_f64_retain(0.999).unwrap_or(Decimal::ZERO);
        let min_val = Decimal::from_f64_retain(-0.999).unwrap_or(Decimal::ZERO);
        if value > max_val {
            value = max_val;
        }
        if value < min_val {
            value = min_val;
        }

        // Fisher = 0.5 * ln((1 + Value) / (1 - Value)) + 0.5 * PreviousFisher
        let num = Decimal::ONE + value;
        let den = Decimal::ONE - value;
        let frac = num / den;
        let ln_val = frac.to_f64().unwrap_or(1.0).ln(); // Convert to f64 for natural log
        let ln_dec = Decimal::from_f64_retain(ln_val).unwrap_or(Decimal::ZERO);

        let half = Decimal::from_f64_retain(0.5).unwrap_or(Decimal::ZERO);
        let fisher = half * ln_dec + half * prev_fisher;

        fisher_vals[i] = Some(fisher.to_f64().unwrap_or(0.0));

        // signal is the previous fisher value
        if i > 0 {
            fisher_signal_vals[i] = fisher_vals[i - 1];
        }

        prev_value = value;
        prev_fisher = fisher;
    }

    Ok((
        Series::new("fisher", fisher_vals),
        Series::new("fisher_signal", fisher_signal_vals),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> DataFrame {
        // Mock data
        let highs = vec![
            Some(10.0),
            Some(12.0),
            Some(15.0),
            Some(14.0),
            Some(13.0),
            Some(16.0),
            Some(18.0),
            Some(17.0),
            Some(20.0),
            Some(22.0),
            Some(21.0),
            Some(19.0),
            Some(18.0),
            Some(15.0),
            Some(14.0),
        ];
        let lows = vec![
            Some(8.0),
            Some(9.0),
            Some(11.0),
            Some(10.0),
            Some(9.0),
            Some(12.0),
            Some(14.0),
            Some(15.0),
            Some(16.0),
            Some(18.0),
            Some(19.0),
            Some(17.0),
            Some(16.0),
            Some(13.0),
            Some(12.0),
        ];

        df!(
            "high" => &highs,
            "low" => &lows
        )
        .unwrap()
    }

    #[test]
    fn test_fisher_transform_basic() {
        let df = create_test_data();
        let (fisher, signal) = calculate(&df, 9).unwrap();

        assert_eq!(fisher.len(), 15);
        assert_eq!(signal.len(), 15);

        let f_vals = fisher.f64().unwrap();
        let s_vals = signal.f64().unwrap();

        // First 8 values should be None
        for i in 0..8 {
            assert!(f_vals.get(i).is_none());
            assert!(s_vals.get(i).is_none());
        }

        // 9th value should be computed, but its signal should be None
        // since signal is prev fisher
        assert!(f_vals.get(8).is_some());
        assert!(s_vals.get(8).is_none());

        // 10th value should be computed and have a signal
        assert!(f_vals.get(9).is_some());
        assert!(s_vals.get(9).is_some());
    }

    #[test]
    fn test_fisher_transform_empty_data() {
        let df = DataFrame::empty();
        let result = calculate(&df, 9);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Data cannot be empty");
    }

    #[test]
    fn test_fisher_transform_zero_period() {
        let df = create_test_data();
        let result = calculate(&df, 0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Period must be greater than 0"
        );
    }

    #[test]
    fn test_fisher_transform_missing_columns() {
        let df = df!("close" => &[10.0, 11.0]).unwrap();
        let result = calculate(&df, 2);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("DataFrame must contain 'high' column"));
    }
}
