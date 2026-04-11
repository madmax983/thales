//! Mass Index
//!
//! The Mass Index is designed to identify trend reversals by measuring the widening
//! and narrowing of the trading range.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::indicators::ema;

/// Calculate Mass Index
///
/// # Arguments
/// * `data` - DataFrame with "high", "low" columns
/// * `ema_period` - Period for the EMAs (typically 9)
/// * `sum_period` - Period for the sum (typically 25)
///
/// # Returns
/// Series with indicator values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::mass_index;
///
/// let df = df!(
///     "high" => &[10.0, 11.0, 12.0, 13.0, 14.0],
///     "low" => &[9.0, 10.0, 11.0, 12.0, 13.0]
/// ).expect("Failed to create dataframe");
///
/// // This will mostly be nulls due to short data length, but shows usage.
/// let result = mass_index::calculate(&df, 9, 25);
/// ```
pub fn calculate(data: &DataFrame, ema_period: usize, sum_period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    let high = data.column("high").context("Missing 'high' column")?;
    let low = data.column("low").context("Missing 'low' column")?;

    let high_ca = high.f64()?;
    let low_ca = low.f64()?;

    // Step 1: True Range (High - Low)
    // Calculate difference directly in Decimal to avoid float math.
    let tr_ca: Float64Chunked = high_ca
        .into_iter()
        .zip(low_ca.into_iter())
        .map(|(h_opt, l_opt)| {
            h_opt.zip(l_opt).and_then(|(h, l)| {
                let h_dec = Decimal::from_f64_retain(h)?;
                let l_dec = Decimal::from_f64_retain(l)?;
                (h_dec - l_dec).to_f64()
            })
        })
        .collect();

    let mut tr_series = tr_ca.into_series();
    tr_series.rename("close");
    let tr_df = DataFrame::new(vec![tr_series])?;

    // Step 2: Single EMA of True Range
    let single_ema = ema::calculate(&tr_df, ema_period)?;
    let mut single_ema_renamed = single_ema.clone();
    single_ema_renamed.rename("close");
    let single_ema_df = DataFrame::new(vec![single_ema_renamed])?;

    // Step 3: Double EMA (EMA of the Single EMA)
    let double_ema = ema::calculate(&single_ema_df, ema_period)?;

    let single_ca = single_ema.f64()?;
    let double_ca = double_ema.f64()?;

    // Step 4: EMA Ratio
    // We compute Single EMA / Double EMA directly in Decimal.
    let ratio_ca: Float64Chunked = single_ca
        .into_iter()
        .zip(double_ca.into_iter())
        .map(|(s_opt, d_opt)| {
            s_opt.zip(d_opt).and_then(|(s, d)| {
                let s_dec = Decimal::from_f64_retain(s)?;
                let d_dec = Decimal::from_f64_retain(d)?;
                if d_dec.is_zero() {
                    None
                } else {
                    (s_dec / d_dec).to_f64()
                }
            })
        })
        .collect();

    // Step 5: Rolling sum of the ratio.
    // Instead of extracting into a loop, we map the lazy DataFrame engine utilizing lazy expressions.
    // The previous approach using `rolling_map` required Polars internals like `RollingOptions`
    // which aren't easily exposed without the `polars-plan` features, nor does rolling_map accept simple closures safely in some versions.
    // Since we must use Decimal, we can achieve this optimally by constructing a simple sliding window map over the extracted Series to a new ChunkedArray avoiding heavy allocations,
    // using the exact same constraints.
    // Let's implement it correctly. `Float64Chunked` has an `into_iter` that yields `Option<f64>`.

    // Convert to Vec safely to provide a continuous slice window without Polars inner complex APIs.
    let ratio_vec: Vec<Option<f64>> = ratio_ca.into_iter().collect();

    // By pre-allocating, and running our Decimal window summation.
    let mut mass_index = Vec::with_capacity(ratio_vec.len());

    for i in 0..ratio_vec.len() {
        if i + 1 < sum_period {
            mass_index.push(None);
            continue;
        }

        let mut sum = Decimal::ZERO;
        let mut valid = true;
        for j in 0..sum_period {
            if let Some(val) = ratio_vec[i - j] {
                if val.is_nan() || val.is_infinite() {
                    valid = false;
                    break;
                }
                if let Some(val_dec) = Decimal::from_f64_retain(val) {
                    sum += val_dec;
                } else {
                    valid = false;
                    break;
                }
            } else {
                valid = false;
                break;
            }
        }

        if valid {
            mass_index.push(sum.to_f64());
        } else {
            mass_index.push(None);
        }
    }

    let mass_index_ca: Float64Chunked = mass_index.into_iter().collect();
    let mut result_series = mass_index_ca.into_series();
    result_series.rename("mass_index");
    Ok(result_series)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_values() -> Result<()> {
        // Calculate Mass Index manually to verify against the implementation.
        // Data: Highs from 10 to 110, Lows from 5 to 105.
        // True Range = 5 constant.
        // EMA of a constant is the constant (after initialization).
        // Double EMA is also the constant.
        // Ratio = 5 / 5 = 1.0.
        // Mass Index for sum_period=25 should converge exactly to 25.0!
        let mut highs = Vec::new();
        let mut lows = Vec::new();

        for i in 0..100 {
            highs.push(10.0 + (i as f64));
            lows.push(5.0 + (i as f64));
        }

        let df = df!(
            "high" => &highs,
            "low" => &lows
        )?;

        let result = calculate(&df, 9, 25)?;
        assert_eq!(result.name(), "mass_index");

        let result_f64 = result.f64()?;

        // Let's check a value well past the initialization period (9 + 9 + 25)
        let val = result_f64.get(80).unwrap();
        // The ratio will approach exactly 1.0. The sum over 25 periods is 25.0.
        assert!((val - 25.0).abs() < 0.001, "Expected ~25.0, got {}", val);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty, 9, 25).is_err());

        let df_single = df!(
            "high" => &[10.0],
            "low" => &[9.0]
        )?;

        let result = calculate(&df_single, 9, 25)?;
        assert_eq!(result.len(), 1);
        let result_f64 = result.f64()?;
        assert!(result_f64.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        // A slightly volatile realistic dataset mock
        let highs = vec![
            105.0, 106.0, 107.0, 108.5, 109.0, 110.0, 109.5, 108.0, 107.5, 106.0, 105.0, 106.5,
            107.0, 108.0, 109.0, 110.5, 111.0, 112.0, 113.0, 114.0, 113.5, 112.0, 111.0, 110.0,
            109.0, 108.0, 107.0, 106.0, 105.0, 106.0, 107.0, 108.0, 109.0, 110.0, 111.0, 112.0,
            113.0, 114.0, 115.0, 116.0, 117.0, 118.0, 119.0, 120.0, 119.5, 118.0, 117.0, 116.0,
        ];
        let lows = vec![
            100.0, 101.0, 102.0, 102.5, 103.0, 104.0, 104.5, 103.0, 102.5, 101.0, 100.0, 101.5,
            102.0, 103.0, 104.0, 104.5, 105.0, 106.0, 107.0, 108.0, 108.5, 107.0, 106.0, 105.0,
            104.0, 103.0, 102.0, 101.0, 100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0,
            108.0, 109.0, 110.0, 111.0, 112.0, 113.0, 114.0, 115.0, 114.5, 113.0, 112.0, 111.0,
        ];

        let df = df!(
            "high" => highs,
            "low" => lows
        )?;

        // Shorten periods to get results within 48 data points
        // ema=3, sum=10. Need ~16 rows for non-null output
        let result = calculate(&df, 3, 10)?;
        let result_f64 = result.f64()?;

        let mut non_null_count = 0;
        for opt_val in result_f64.into_iter() {
            if opt_val.is_some() {
                non_null_count += 1;
            }
        }

        // Ensure we got some actual calculated indicator values
        assert!(non_null_count > 0, "Realistic data test failed to produce non-null output");

        Ok(())
    }
}
