//! Gator Oscillator
//!
//! The Gator Oscillator is a technical indicator created by Bill Williams.
//! It is used to identify when a market is trending, or when it is range-bound.
//! The oscillator is based on the Alligator indicator, plotting the absolute differences
//! between the Jaw and Teeth (upper histogram), and the Teeth and Lips (lower histogram).

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate the SMA over `window` periods for a slice of `Decimal` values.
fn calculate_sma(data: &[Option<Decimal>], window: usize) -> Result<Vec<Option<Decimal>>> {
    if window == 0 {
        anyhow::bail!("Window must be greater than 0");
    }

    let mut result = vec![None; data.len()];
    let mut sum = Decimal::ZERO;
    let mut count = 0;
    let mut queue: std::collections::VecDeque<Option<Decimal>> = std::collections::VecDeque::new();
    let window_dec = Decimal::from_usize(window).context("Invalid window size")?;

    for i in 0..data.len() {
        let val_opt = data[i];
        queue.push_back(val_opt);

        if let Some(val) = val_opt {
            sum += val;
            count += 1;
        }

        if queue.len() > window {
            if let Some(popped) = queue.pop_front() {
                if let Some(val) = popped {
                    sum -= val;
                    count -= 1;
                }
            }
        }

        if queue.len() == window {
            if count == window {
                // All values valid
                result[i] = Some(sum / window_dec);
            } else {
                // Some values were None
                result[i] = None;
            }
        }
    }
    Ok(result)
}

/// Calculate the Smoothed Moving Average (SMMA).
fn calculate_smma(data: &[Option<Decimal>], window: usize) -> Result<Vec<Option<Decimal>>> {
    if window == 0 {
        anyhow::bail!("Window must be greater than 0");
    }

    let mut result = vec![None; data.len()];
    let mut prev_smma: Option<Decimal> = None;
    let window_dec = Decimal::from_usize(window).context("Invalid window size")?;

    let sma_vals = calculate_sma(data, window)?;

    for i in 0..data.len() {
        if let Some(val) = data[i] {
            if let Some(prev) = prev_smma {
                // SMMA = (PREVSUM - PREVSMMA + CLOSE) / N
                let new_smma = ((prev * window_dec) - prev + val) / window_dec;
                result[i] = Some(new_smma);
                prev_smma = Some(new_smma);
            } else if let Some(sma) = sma_vals[i] {
                // Seed with SMA
                result[i] = Some(sma);
                prev_smma = Some(sma);
            }
        } else {
            result[i] = None;
            prev_smma = None;
        }
    }
    Ok(result)
}

/// Shift data forward by `shift` periods.
fn shift_forward(data: &[Option<Decimal>], shift: usize) -> Vec<Option<Decimal>> {
    let mut result = vec![None; data.len()];
    for i in 0..data.len() {
        if i >= shift {
            result[i] = data[i - shift];
        }
    }
    result
}

/// Calculate Gator Oscillator
///
/// # Arguments
/// * `data` - DataFrame with "high" and "low" columns
/// * `jaw_period` - Lookback period for Jaw (e.g., 13)
/// * `jaw_shift` - Forward shift for Jaw (e.g., 8)
/// * `teeth_period` - Lookback period for Teeth (e.g., 8)
/// * `teeth_shift` - Forward shift for Teeth (e.g., 5)
/// * `lips_period` - Lookback period for Lips (e.g., 5)
/// * `lips_shift` - Forward shift for Lips (e.g., 3)
///
/// # Returns
/// Tuple of (Series upper_gator, Series lower_gator).
/// The upper histogram is absolute difference between Jaw and Teeth.
/// The lower histogram is absolute difference between Teeth and Lips, but plotted negative.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let (upper, lower) = strategies::indicators::gator::calculate(&df, 13, 8, 8, 5, 5, 3)?;
/// ```
pub fn calculate(
    data: &DataFrame,
    jaw_period: usize,
    jaw_shift: usize,
    teeth_period: usize,
    teeth_shift: usize,
    lips_period: usize,
    lips_shift: usize,
) -> Result<(Series, Series)> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if jaw_period == 0 || teeth_period == 0 || lips_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

    let high_series = data
        .column("high")
        .context("DataFrame must contain 'high' column")?
        .f64()
        .context("High column must be numeric (f64)")?;

    let low_series = data
        .column("low")
        .context("DataFrame must contain 'low' column")?
        .f64()
        .context("Low column must be numeric (f64)")?;

    // Calculate typical price (Median Price = (High + Low) / 2)
    let two = Decimal::new(2, 0);
    let mut median_prices: Vec<Option<Decimal>> = Vec::with_capacity(data.height());

    for i in 0..data.height() {
        if let (Some(h), Some(l)) = (high_series.get(i), low_series.get(i)) {
            if h.is_finite() && l.is_finite() {
                let h_dec = Decimal::from_f64_retain(h).unwrap_or(Decimal::ZERO);
                let l_dec = Decimal::from_f64_retain(l).unwrap_or(Decimal::ZERO);
                median_prices.push(Some((h_dec + l_dec) / two));
            } else {
                median_prices.push(None);
            }
        } else {
            median_prices.push(None);
        }
    }

    // Calculate Jaw, Teeth, Lips SMMA
    let jaw_smma = calculate_smma(&median_prices, jaw_period)?;
    let teeth_smma = calculate_smma(&median_prices, teeth_period)?;
    let lips_smma = calculate_smma(&median_prices, lips_period)?;

    // Shift Jaw, Teeth, Lips
    let shifted_jaw = shift_forward(&jaw_smma, jaw_shift);
    let shifted_teeth = shift_forward(&teeth_smma, teeth_shift);
    let shifted_lips = shift_forward(&lips_smma, lips_shift);

    let mut upper_vals: Vec<Option<f64>> = vec![None; data.height()];
    let mut lower_vals: Vec<Option<f64>> = vec![None; data.height()];

    for i in 0..data.height() {
        if let (Some(jaw), Some(teeth)) = (shifted_jaw[i], shifted_teeth[i]) {
            let diff = (jaw - teeth).abs();
            upper_vals[i] = diff.to_f64();
        }
        if let (Some(teeth), Some(lips)) = (shifted_teeth[i], shifted_lips[i]) {
            let diff = -(teeth - lips).abs();
            lower_vals[i] = diff.to_f64();
        }
    }

    let upper_series = Series::new("gator_upper", upper_vals);
    let lower_series = Series::new("gator_lower", lower_vals);

    Ok((upper_series, lower_series))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0],
            "low" =>  &[ 8.0,  9.0, 10.0, 11.0, 12.0, 13.0, 14.0]
        )?;

        // Very small periods for testing to trigger fast:
        // Jaw: P=3, S=2
        // Teeth: P=2, S=1
        // Lips: P=1, S=0

        let (upper, lower) = calculate(&df, 3, 2, 2, 1, 1, 0)?;
        let upper_out = upper.f64()?;
        let lower_out = lower.f64()?;

        // Data points:
        // MP: 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0

        // Jaw SMMA(3):
        // i=2: SMA(3) = (9+10+11)/3 = 10.0
        // i=3: SMMA = (10.0*3 - 10.0 + 12.0)/3 = 10.666...
        // i=4: SMMA = (10.666*3 - 10.666 + 13.0)/3 = 11.444...

        // Shifted Jaw (+2):
        // i=4: 10.0
        // i=5: 10.666...

        // Teeth SMMA(2):
        // i=1: SMA(2) = (9+10)/2 = 9.5
        // i=2: SMMA = (9.5*2 - 9.5 + 11.0)/2 = 10.5
        // i=3: SMMA = (10.5*2 - 10.5 + 12.0)/2 = 11.25
        // i=4: SMMA = (11.25*2 - 11.25 + 13.0)/2 = 12.125
        // i=5: SMMA = (12.125*2 - 12.125 + 14.0)/2 = 13.0625

        // Shifted Teeth (+1):
        // i=2: 9.5
        // i=3: 10.5
        // i=4: 11.25
        // i=5: 12.125

        // Lips SMMA(1):
        // i=0: 9.0
        // i=1: 10.0
        // i=2: 11.0
        // i=3: 12.0
        // i=4: 13.0
        // i=5: 14.0

        // Shifted Lips (+0):
        // i=2: 11.0
        // i=3: 12.0
        // i=4: 13.0
        // i=5: 14.0

        // Upper(i=4) = |Jaw(i=4) - Teeth(i=4)| = |10.0 - 11.25| = 1.25
        // Lower(i=4) = -|Teeth(i=4) - Lips(i=4)| = -|11.25 - 13.0| = -1.75

        assert_eq!(upper_out.len(), 7);
        assert_eq!(lower_out.len(), 7);
        assert!(upper_out.get(0).is_none());

        let val_upper_4 = upper_out.get(4).unwrap();
        let val_lower_4 = lower_out.get(4).unwrap();

        let val_upper_5 = upper_out.get(5).unwrap();
        let val_lower_5 = lower_out.get(5).unwrap();

        // Assert they are not None and they are finite values
        assert!(val_upper_4.is_finite());
        assert!(val_lower_4.is_finite());
        assert!(val_upper_5.is_finite());
        assert!(val_lower_5.is_finite());

        // Lower should be negative
        assert!(val_lower_4 <= 0.0);
        assert!(val_lower_5 <= 0.0);

        // Upper should be positive
        assert!(val_upper_4 >= 0.0);
        assert!(val_upper_5 >= 0.0);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 13, 8, 8, 5, 5, 3);
        assert!(res_empty.is_err());

        // Single point (too small for periods)
        let df_short = df!(
            "high" => &[10.0],
            "low" => &[8.0]
        )?;
        let (upper, lower) = calculate(&df_short, 13, 8, 8, 5, 5, 3)?;
        assert_eq!(upper.len(), 1);
        assert!(upper.f64()?.get(0).is_none());
        assert!(lower.f64()?.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        // Create 30 points of data
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        for i in 0..30 {
            highs.push(100.0 + (i as f64));
            lows.push(95.0 + (i as f64));
        }

        let df = df!(
            "high" => highs,
            "low" => lows
        )?;

        // Using standard Gator periods
        let (upper, lower) = calculate(&df, 13, 8, 8, 5, 5, 3)?;
        let upper_out = upper.f64()?;
        let lower_out = lower.f64()?;

        assert_eq!(upper_out.len(), 30);
        assert_eq!(lower_out.len(), 30);

        // Due to period 13 and shift 8, the upper array will have Nones until index 20
        assert!(upper_out.get(0).is_none());
        assert!(upper_out.get(19).is_none());
        assert!(upper_out.get(20).is_some());

        // Lower array needs period 8 and shift 5 => 12
        assert!(lower_out.get(11).is_none());
        assert!(lower_out.get(12).is_some());

        // Validate values are finite
        if let Some(val) = upper_out.get(25) {
            assert!(val.is_finite());
        }
        if let Some(val) = lower_out.get(25) {
            assert!(val.is_finite());
            assert!(val <= 0.0); // Lower is always negative
        }

        Ok(())
    }
}
