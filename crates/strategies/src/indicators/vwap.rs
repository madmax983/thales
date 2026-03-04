//! Volume Weighted Average Price (VWAP)
//!
//! A trading benchmark that gives the average price a security has traded at
//! throughout the day, based on both volume and price. It resets at the beginning
//! of each trading session (or day).

use anyhow::{Context, Result};
use chrono::{Datelike, TimeZone, Utc};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate volume spreading (VWAP) (Volume Weighted Average Price)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close", "volume", and "timestamp_unix_ms" columns
///
/// # Returns
/// Series with VWAP values.
///
/// # Example
/// ```rust
/// use strategies::indicators::vwap;
/// use polars::prelude::*;
///
/// // Assuming df is a DataFrame with required columns
/// // let result = vwap::calculate(&df)?;
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
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

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let volume = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?
        .f64()
        .context("Volume column must be numeric (f64)")?;

    let timestamp = data
        .column("timestamp_unix_ms")
        .context("DataFrame must contain 'timestamp_unix_ms' column")?
        .i64()
        .context("Timestamp column must be numeric (i64)")?;

    let mut vwap_values: Vec<Option<f64>> = Vec::with_capacity(close.len());

    let mut sum_price_vol = Decimal::ZERO;
    let mut sum_vol = Decimal::ZERO;
    let mut current_day: Option<u32> = None;

    let three = Decimal::from(3);

    let high_iter = high.into_iter();
    let low_iter = low.into_iter();
    let close_iter = close.into_iter();
    let volume_iter = volume.into_iter();
    let timestamp_iter = timestamp.into_iter();

    for ((((h_opt, l_opt), c_opt), v_opt), t_opt) in high_iter
        .zip(low_iter)
        .zip(close_iter)
        .zip(volume_iter)
        .zip(timestamp_iter)
    {
        match (h_opt, l_opt, c_opt, v_opt, t_opt) {
            (Some(h), Some(l), Some(c), Some(v), Some(t)) => {
                // Determine day of year to reset VWAP
                if let polars::export::chrono::LocalResult::Single(dt) = Utc.timestamp_millis_opt(t)
                {
                    let day_of_year = dt.ordinal();

                    if current_day.is_none() || current_day != Some(day_of_year) {
                        // Reset accumulators for the new day
                        sum_price_vol = Decimal::ZERO;
                        sum_vol = Decimal::ZERO;
                        current_day = Some(day_of_year);
                    }

                    if let (Some(h_dec), Some(l_dec), Some(c_dec), Some(v_dec)) = (
                        Decimal::from_f64_retain(h),
                        Decimal::from_f64_retain(l),
                        Decimal::from_f64_retain(c),
                        Decimal::from_f64_retain(v),
                    ) {
                        let typical_price = (h_dec + l_dec + c_dec) / three;
                        let price_vol = typical_price * v_dec;

                        sum_price_vol += price_vol;
                        sum_vol += v_dec;

                        if sum_vol == Decimal::ZERO {
                            vwap_values.push(Some(c)); // fallback to close price
                        } else {
                            let avg = sum_price_vol.checked_div(sum_vol).unwrap_or(Decimal::ZERO);
                            vwap_values.push(avg.to_f64());
                        }
                    } else {
                        vwap_values.push(None);
                    }
                } else {
                    vwap_values.push(None);
                }
            }
            _ => {
                // Missing data point, keep accumulators but emit None
                vwap_values.push(None);
            }
        }
    }

    let s = Series::new("vwap", vwap_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let dt1 = Utc
            .with_ymd_and_hms(2023, 1, 1, 10, 0, 0)
            .unwrap()
            .timestamp_millis();
        let dt2 = Utc
            .with_ymd_and_hms(2023, 1, 1, 11, 0, 0)
            .unwrap()
            .timestamp_millis();
        let dt3 = Utc
            .with_ymd_and_hms(2023, 1, 1, 12, 0, 0)
            .unwrap()
            .timestamp_millis();

        let df = df!(
            "high" => &[10.0, 12.0, 14.0],
            "low" => &[10.0, 8.0, 10.0],
            "close" => &[10.0, 10.0, 12.0],
            "volume" => &[100.0, 200.0, 100.0],
            "timestamp_unix_ms" => &[dt1, dt2, dt3]
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        // Bar 1: TP = 10. PV = 1000. Sum_PV = 1000, Sum_V = 100. VWAP = 10.0.
        // Bar 2: TP = 10. PV = 2000. Sum_PV = 3000, Sum_V = 300. VWAP = 10.0.
        // Bar 3: TP = 12. PV = 1200. Sum_PV = 4200, Sum_V = 400. VWAP = 10.5.

        assert_eq!(out.get(0), Some(10.0));
        assert_eq!(out.get(1), Some(10.0));
        assert_eq!(out.get(2), Some(10.5));

        Ok(())
    }

    #[test]
    fn test_daily_reset() -> Result<()> {
        let dt1 = Utc
            .with_ymd_and_hms(2023, 1, 1, 23, 0, 0)
            .unwrap()
            .timestamp_millis();
        let dt2 = Utc
            .with_ymd_and_hms(2023, 1, 2, 1, 0, 0)
            .unwrap()
            .timestamp_millis();

        let df = df!(
            "high" => &[10.0, 20.0],
            "low" => &[10.0, 20.0],
            "close" => &[10.0, 20.0],
            "volume" => &[100.0, 100.0],
            "timestamp_unix_ms" => &[dt1, dt2]
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        // Day 1: TP = 10. VWAP = 10.0.
        // Day 2 (Reset): TP = 20. VWAP = 20.0.

        assert_eq!(out.get(0), Some(10.0));
        assert_eq!(out.get(1), Some(20.0));

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Missing column
        let df_missing = df!("high" => &[10.0])?;
        let res_missing = calculate(&df_missing);
        assert!(res_missing.is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut high = Vec::new();
        let mut low = Vec::new();
        let mut close = Vec::new();
        let mut volume = Vec::new();
        let mut timestamp = Vec::new();

        let start_dt = Utc
            .with_ymd_and_hms(2023, 1, 1, 9, 30, 0)
            .unwrap()
            .timestamp_millis();

        // Generate 100 periods of simulated trading data for a single day
        let mut current_price = 100.0;
        for i in 0..100 {
            high.push(current_price + 1.0);
            low.push(current_price - 1.0);
            close.push(current_price);
            volume.push(1000.0 + (i as f64) * 10.0);
            timestamp.push(start_dt + (i as i64) * 60000); // 1 minute intervals

            // Create a slight uptrend
            current_price += 0.1;
        }

        let df = df!(
            "high" => &high,
            "low" => &low,
            "close" => &close,
            "volume" => &volume,
            "timestamp_unix_ms" => &timestamp
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 100);

        // Check that the VWAP generally tracks the typical price upwards
        let first_vwap = out.get(0).unwrap();
        let last_vwap = out.get(99).unwrap();

        assert!(last_vwap > first_vwap);
        assert!((first_vwap - 100.0).abs() < 1e-6);

        Ok(())
    }
}
