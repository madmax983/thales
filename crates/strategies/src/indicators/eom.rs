//! Ease of Movement (EOM)
//!
//! Relates an asset's price change to its volume. It helps identify how easily a price can move up or down based on volume.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use crate::indicators::sma;

/// Calculate Ease of Movement (EOM)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", and "volume" columns
/// * `period` - Lookback period for smoothing
///
/// # Returns
/// Series with indicator values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::eom;
///
/// let df = df!(
///     "high" => &[10.0, 11.0, 12.0],
///     "low" => &[9.0, 10.0, 11.0],
///     "volume" => &[1000.0, 1200.0, 1500.0]
/// ).unwrap();
/// let result = eom::calculate(&df, 2);
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let high_col = data
        .column("high")
        .context("DataFrame must contain 'high' column")?
        .f64()
        .context("High column must be numeric (f64)")?;

    let low_col = data
        .column("low")
        .context("DataFrame must contain 'low' column")?
        .f64()
        .context("Low column must be numeric (f64)")?;

    let volume_col = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?
        .f64()
        .context("Volume column must be numeric (f64)")?;

    let len = data.height();
    let mut eom_1_period_values: Vec<Option<f64>> = vec![None; len];

    let decimal_two = Decimal::from(2);
    let decimal_divisor = Decimal::from(100_000_000); // Usually volume is scaled by 10,000, 100,000, or 100_000_000. Standard EOM formula uses 100,000_000.

    for (i, val) in eom_1_period_values.iter_mut().enumerate().take(len).skip(1) {
        let curr_high_opt = high_col.get(i);
        let curr_low_opt = low_col.get(i);
        let curr_vol_opt = volume_col.get(i);

        let prev_high_opt = high_col.get(i - 1);
        let prev_low_opt = low_col.get(i - 1);

        if let (Some(curr_high), Some(curr_low), Some(curr_vol), Some(prev_high), Some(prev_low)) = (
            curr_high_opt,
            curr_low_opt,
            curr_vol_opt,
            prev_high_opt,
            prev_low_opt,
        ) {
            let ch = Decimal::from_f64_retain(curr_high);
            let cl = Decimal::from_f64_retain(curr_low);
            let cv = Decimal::from_f64_retain(curr_vol);
            let ph = Decimal::from_f64_retain(prev_high);
            let pl = Decimal::from_f64_retain(prev_low);

            if let (Some(ch), Some(cl), Some(cv), Some(ph), Some(pl)) = (ch, cl, cv, ph, pl) {
                // Distance Moved = ((Current High + Current Low) / 2) - ((Prior High + Prior Low) / 2)
                let curr_midpoint = (ch + cl) / decimal_two;
                let prev_midpoint = (ph + pl) / decimal_two;
                let distance_moved = curr_midpoint - prev_midpoint;

                let range = ch - cl;
                if range != Decimal::ZERO && cv != Decimal::ZERO {
                    // Box Ratio = (Volume / 100,000,000) / (High - Low)
                    let volume_scaled = cv / decimal_divisor;
                    let box_ratio = volume_scaled / range;

                    if box_ratio != Decimal::ZERO {
                        let eom_1 = distance_moved / box_ratio;
                        *val = eom_1.to_f64();
                    } else {
                        *val = Some(0.0);
                    }
                } else {
                    *val = Some(0.0);
                }
            }
        }
    }

    let eom_1_series = Series::new("close", eom_1_period_values);
    let temp_df = DataFrame::new(vec![eom_1_series])?;

    // N-Period EOM = SMA of 1-Period EOM over period
    let mut sma_series = sma::calculate(&temp_df, period)?;
    sma_series.rename("eom");

    Ok(sma_series)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 11.0, 12.0, 13.0, 14.0],
            "low" => &[9.0, 10.0, 11.0, 12.0, 13.0],
            "volume" => &[100_000_000.0, 200_000_000.0, 150_000_000.0, 250_000_000.0, 100_000_000.0]
        )?;

        // Period 2.
        // i=0: None
        // i=1:
        // ch=11, cl=10, cv=200M, ph=10, pl=9.
        // curr_mid = 10.5, prev_mid = 9.5 -> DM = 1.0
        // range = 1.0, vol_scaled = 2.0 -> BR = 2.0
        // EOM_1 = 1.0 / 2.0 = 0.5

        // i=2:
        // ch=12, cl=11, cv=150M, ph=11, pl=10
        // DM = 1.0
        // range = 1.0, vol_scaled = 1.5 -> BR = 1.5
        // EOM_1 = 1.0 / 1.5 = 0.66666...

        // SMA period=2 on [None, 0.5, 0.666...]
        // SMA at i=0: None
        // SMA at i=1: None
        // SMA at i=2: (0.5 + 0.666...) / 2 = 0.58333...

        let result = calculate(&df, 2)?;
        let result_vals = result.f64()?;

        assert!(result_vals.get(0).is_none());
        assert!(result_vals.get(1).is_none());

        let val2 = result_vals.get(2).unwrap();
        assert!((val2 - 0.5833).abs() < 0.001);

        Ok(())
    }

    #[test]
    fn test_edge_cases() {
        let df_empty = DataFrame::default();
        let result = calculate(&df_empty, 14);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Data cannot be empty");

        let df_single = df!(
            "high" => &[10.0],
            "low" => &[9.0],
            "volume" => &[1000.0]
        )
        .unwrap();
        let result_single = calculate(&df_single, 14);
        // Will be ok but just None values
        assert!(result_single.is_ok());
        let vals = result_single.unwrap();
        assert_eq!(vals.len(), 1);
        assert!(vals.f64().unwrap().get(0).is_none());

        let df_missing_col = df!(
            "high" => &[10.0, 11.0],
            "low" => &[9.0, 10.0]
        )
        .unwrap();
        let result_missing = calculate(&df_missing_col, 2);
        assert!(result_missing.is_err());
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut high_vals = vec![];
        let mut low_vals = vec![];
        let mut vol_vals = vec![];

        let mut current_price = 100.0;
        for i in 0..50 {
            high_vals.push(current_price + 1.0);
            low_vals.push(current_price - 1.0);
            vol_vals.push(100_000_000.0 + (i as f64 * 1_000_000.0));
            current_price += 0.5;
        }

        let df = df!(
            "high" => &high_vals,
            "low" => &low_vals,
            "volume" => &vol_vals
        )?;

        let result = calculate(&df, 14)?;
        assert_eq!(result.len(), 50);
        let vals = result.f64()?;

        // The first 14 values should be None (SMA period 14 on 1-period shifted EOM requires 14 valid data points).
        // Since EOM_1 starts at index 1, SMA of period 14 needs data points 1 to 14. So index 14 is the first valid SMA.
        assert!(vals.get(0).is_none());
        assert!(vals.get(13).is_none());
        assert!(vals.get(14).is_some());

        Ok(())
    }
}
