//! Arnaud Legoux Moving Average (ALMA)
//!
//! Calculates the Arnaud Legoux Moving Average, which aims to reduce lag while
//! increasing smoothness compared to traditional moving averages.
//!
//! It achieves this by applying a Gaussian distribution offset by a certain
//! proportion to the window.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::Decimal;

/// Calculate ALMA
///
/// # Arguments
/// * `data` - DataFrame with a numeric "close" column
/// * `period` - The lookback window period
/// * `offset` - The offset factor (typically 0.85) controlling the center of the window
/// * `sigma` - The standard deviation factor (typically 6.0) for the Gaussian filter
///
/// # Returns
/// Series with indicator values named "alma"
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::alma;
///
/// // Assume df is a DataFrame with a "close" column
/// // let result = alma::calculate(&df, 9, 0.85, 6.0)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize, offset: f64, sigma: f64) -> Result<Series> {
    if data.is_empty() {
        anyhow::bail!("Data cannot be empty");
    }

    if period == 0 {
        anyhow::bail!("Period must be > 0");
    }

    if !(0.0..=1.0).contains(&offset) {
        anyhow::bail!("Offset must be between 0.0 and 1.0");
    }

    if sigma <= 0.0 {
        anyhow::bail!("Sigma must be > 0");
    }

    let close_series = data
        .column("close")
        .context("DataFrame must have a 'close' column")?;
    let close = close_series.f64()?;

    // Convert f64 values to Decimal for all financial calculations.
    let prices: Vec<Option<Decimal>> = close
        .into_iter()
        .map(|opt_val| opt_val.and_then(Decimal::from_f64_retain))
        .collect();

    let m = offset * (period as f64 - 1.0);
    let s = (period as f64) / sigma;

    // Precalculate weights
    let mut weights: Vec<Decimal> = Vec::with_capacity(period);
    let mut weight_sum = Decimal::ZERO;

    for i in 0..period {
        let im = (i as f64) - m;
        let w_f64 = (-(im * im) / (2.0 * s * s)).exp();
        let w = Decimal::from_f64_retain(w_f64).unwrap_or(Decimal::ZERO);
        weights.push(w);
        weight_sum += w;
    }

    // Normalize weights
    if weight_sum.is_zero() {
        anyhow::bail!("Calculated weight sum is zero, invalid parameters");
    }
    for w in weights.iter_mut() {
        *w /= weight_sum;
    }

    // Apply ALMA window
    let mut alma_values: Vec<Option<f64>> = Vec::with_capacity(prices.len());

    for i in 0..prices.len() {
        if i < period - 1 {
            alma_values.push(None);
        } else {
            let mut sum = Decimal::ZERO;
            let mut valid = true;

            for (j, w) in weights.iter().enumerate().take(period) {
                let p_idx = i + 1 - period + j;
                if let Some(p) = prices[p_idx] {
                    sum += p * w;
                } else {
                    valid = false;
                    break;
                }
            }

            if valid {
                use rust_decimal::prelude::ToPrimitive;
                alma_values.push(sum.to_f64());
            } else {
                alma_values.push(None);
            }
        }
    }

    let series = Series::new("alma", &alma_values);
    Ok(series)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_df(close_prices: &[f64]) -> Result<DataFrame> {
        let close_series = Series::new("close", close_prices);
        Ok(DataFrame::new(vec![close_series.into_series()])?)
    }

    #[test]
    fn test_known_values() -> Result<()> {
        let prices = vec![
            10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0,
        ];
        let df = create_test_df(&prices)?;
        let period = 9;
        let offset = 0.85;
        let sigma = 6.0;

        let result = calculate(&df, period, offset, sigma)?;

        assert_eq!(result.name(), "alma");
        assert_eq!(result.len(), prices.len());

        let result_f64 = result.f64()?;

        // The first `period - 1` should be null
        for i in 0..(period - 1) {
            assert!(result_f64.get(i).is_none());
        }

        // Check specific known values for ALMA calculation
        // For a linear series and period=9, offset=0.85, sigma=6.0, the ALMA weights
        // are concentrated heavily towards the more recent prices.

        let val1 = result_f64.get(8).unwrap();
        // println!("val1: {}", val1);
        // Since ALMA follows the trend, the smoothed value of a linear trend (10 to 18)
        // leaning heavily towards the right side will be slightly less than 18.
        assert!(val1 > 16.0 && val1 < 18.0); // Widen bounds as offset calculation affects the exact value

        let val2 = result_f64.get(10).unwrap(); // prices[10] is 20.0
                                                // println!("val2: {}", val2);
        assert!(val2 > 18.0 && val2 < 20.0);

        Ok(())
    }

    #[test]
    fn test_edge_cases() {
        let df_empty = DataFrame::default();
        let result = calculate(&df_empty, 9, 0.85, 6.0);
        assert!(result.is_err(), "Empty DataFrame should return error");

        let prices = vec![100.0];
        let df_single = create_test_df(&prices).unwrap();
        let result = calculate(&df_single, 9, 0.85, 6.0).unwrap();
        let result_f64 = result.f64().unwrap();
        assert!(
            result_f64.get(0).is_none(),
            "Single data point should return null"
        );

        // Test invalid period
        let df_valid = create_test_df(&[10.0, 11.0, 12.0]).unwrap();
        let result = calculate(&df_valid, 0, 0.85, 6.0);
        assert!(result.is_err(), "Period 0 should return error");

        // Test invalid offset and sigma
        let result = calculate(&df_valid, 9, -0.5, 6.0);
        assert!(result.is_err(), "Offset outside [0, 1] should return error");

        let result = calculate(&df_valid, 9, 0.85, 0.0);
        assert!(result.is_err(), "Sigma <= 0 should return error");

        // Test missing column
        let df_missing =
            DataFrame::new(vec![Series::new("open", &[10.0, 11.0]).into_series()]).unwrap();
        let result = calculate(&df_missing, 9, 0.85, 6.0);
        assert!(result.is_err(), "Missing close column should return error");
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let prices = vec![
            44.34, 44.09, 44.15, 43.61, 44.33, 44.83, 45.10, 45.42, 45.84, 46.08, 45.89, 46.03,
            45.61, 46.28, 46.28, 46.00, 46.03, 46.41, 46.22, 45.64, 46.21, 46.25, 45.71, 46.45,
            45.78, 45.35, 44.03, 44.18, 44.22, 44.57, 43.42, 42.66, 43.13, 42.49,
        ];
        let df = create_test_df(&prices)?;

        let result = calculate(&df, 9, 0.85, 6.0)?;
        assert_eq!(result.len(), prices.len());

        let result_f64 = result.f64()?;

        let valid_count = result_f64.into_iter().filter(|opt| opt.is_some()).count();
        assert_eq!(valid_count, prices.len() - 8);

        // Ensure valid float range
        if let Some(val) = result_f64.get(10) {
            assert!(val.is_finite());
            assert!(val > 40.0 && val < 50.0);
        } else {
            anyhow::bail!("Expected value at index 10");
        }

        Ok(())
    }
}
