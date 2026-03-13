//! Force Index (FI)
//!
//! Alexander Elder's Force Index combines price movement and volume to measure
//! the strength of bulls and bears in the market.

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use super::ema;

/// Calculate Force Index
///
/// # Arguments
/// * `data` - DataFrame with "close", "volume", and "timestamp_unix_ms" columns
/// * `period` - Lookback period for EMA smoothing (typically 13)
///
/// # Returns
/// Series with Force Index values.
///
/// # Example
/// ```rust
/// use strategies::indicators::force_index;
/// use polars::prelude::*;
///
/// // Assuming df is a DataFrame with "close" and "volume" columns
/// // let result = force_index::calculate(&df, 13)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

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

    let timestamps = data
        .column("timestamp_unix_ms")
        .context("DataFrame must contain 'timestamp_unix_ms' column")?
        .i64()
        .context("Timestamp column must be numeric (i64)")?;

    let mut raw_fi_values: Vec<Option<f64>> = Vec::with_capacity(close.len());

    let mut prev_close: Option<Decimal> = None;

    let close_iter = close.into_iter();
    let vol_iter = volume.into_iter();
    let ts_iter = timestamps.into_iter();

    for ((c_opt, v_opt), ts_opt) in close_iter.zip(vol_iter).zip(ts_iter) {
        match (c_opt, v_opt, ts_opt) {
            (Some(c_f), Some(v_f), Some(ts)) => {
                // Ensure timestamp is valid DateTime<Utc>
                let _dt: DateTime<Utc> = match Utc.timestamp_millis_opt(ts) {
                    polars::export::chrono::LocalResult::Single(dt) => dt,
                    _ => {
                        raw_fi_values.push(None);
                        prev_close = None;
                        continue;
                    }
                };

                if let (Some(c), Some(v)) =
                    (Decimal::from_f64_retain(c_f), Decimal::from_f64_retain(v_f))
                {
                    if let Some(pc) = prev_close {
                        let fi = (c - pc) * v;
                        raw_fi_values.push(fi.to_f64());
                    } else {
                        raw_fi_values.push(None);
                    }
                    prev_close = Some(c);
                } else {
                    raw_fi_values.push(None);
                    prev_close = None;
                }
            }
            _ => {
                raw_fi_values.push(None);
                prev_close = None;
            }
        }
    }

    let raw_fi_series = Series::new("close".into(), raw_fi_values);
    let temp_df = DataFrame::new(vec![raw_fi_series])?;

    if period == 1 {
        let mut s = temp_df.column("close")?.clone();
        s.rename("force_index".into());
        return Ok(s);
    }

    let mut ema_series = ema::calculate(&temp_df, period)?;
    ema_series.rename("force_index".into());

    Ok(ema_series)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_force_index_calculation() -> Result<()> {
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
        let dt4 = Utc
            .with_ymd_and_hms(2023, 1, 1, 13, 0, 0)
            .unwrap()
            .timestamp_millis();
        let dt5 = Utc
            .with_ymd_and_hms(2023, 1, 1, 14, 0, 0)
            .unwrap()
            .timestamp_millis();

        let df = df!(
            "close" => &[10.0, 12.0, 11.0, 13.0, 15.0],
            "volume" => &[100.0, 200.0, 150.0, 300.0, 250.0],
            "timestamp_unix_ms" => &[dt1, dt2, dt3, dt4, dt5]
        )?;

        // Raw FI:
        // 0: None
        // 1: (12 - 10) * 200 = 400.0
        // 2: (11 - 12) * 150 = -150.0
        // 3: (13 - 11) * 300 = 600.0
        // 4: (15 - 13) * 250 = 500.0

        let result = calculate(&df, 1)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert_eq!(out.get(1), Some(400.0));
        assert_eq!(out.get(2), Some(-150.0));
        assert_eq!(out.get(3), Some(600.0));
        assert_eq!(out.get(4), Some(500.0));

        // EMA of Period 3:
        // values: [None, 400, -150, 600, 500]
        // K = 2 / 4 = 0.5
        // SMA seed (3 periods: 400, -150, 600) = 850 / 3 = 283.333...
        // Index 3 (600) will be the seed.
        // Index 4 (500) will be EMA: (500 * 0.5) + (283.333 * 0.5) = 250 + 141.666 = 391.666...

        let result_ema = calculate(&df, 3)?;
        let out_ema = result_ema.f64()?;

        assert!(out_ema.get(0).is_none());
        assert!(out_ema.get(1).is_none());
        assert!(out_ema.get(2).is_none());

        // Seed value at index 3
        let seed = (400.0 - 150.0 + 600.0) / 3.0;
        if let Some(val) = out_ema.get(3) {
            assert!((val - seed).abs() < 1e-6);
        } else {
            anyhow::bail!("Expected value at index 3");
        }

        // EMA value at index 4
        let expected_ema = (500.0 * 0.5) + (seed * 0.5);
        if let Some(val) = out_ema.get(4) {
            assert!((val - expected_ema).abs() < 1e-6);
        } else {
            anyhow::bail!("Expected value at index 4");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 13);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        // Missing column
        let df_missing = df!("close" => &[10.0], "volume" => &[100.0])?;
        let res_missing = calculate(&df_missing, 13);
        assert!(res_missing.is_err());

        // Zero period
        let dt1 = Utc
            .with_ymd_and_hms(2023, 1, 1, 10, 0, 0)
            .unwrap()
            .timestamp_millis();
        let df_valid =
            df!("close" => &[10.0], "volume" => &[100.0], "timestamp_unix_ms" => &[dt1])?;
        let res_zero = calculate(&df_valid, 0);
        assert!(res_zero.is_err());
        assert_eq!(
            res_zero.unwrap_err().to_string(),
            "Period must be greater than 0"
        );

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
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
        let dt4 = Utc
            .with_ymd_and_hms(2023, 1, 1, 13, 0, 0)
            .unwrap()
            .timestamp_millis();
        let dt5 = Utc
            .with_ymd_and_hms(2023, 1, 1, 14, 0, 0)
            .unwrap()
            .timestamp_millis();
        let dt6 = Utc
            .with_ymd_and_hms(2023, 1, 1, 15, 0, 0)
            .unwrap()
            .timestamp_millis();
        let dt7 = Utc
            .with_ymd_and_hms(2023, 1, 1, 16, 0, 0)
            .unwrap()
            .timestamp_millis();

        let df = df!(
            "close" => &[100.0, 101.0, 102.0, 101.5, 103.0, 104.0, 103.5],
            "volume" => &[1000.0, 1100.0, 1200.0, 900.0, 1500.0, 1600.0, 1000.0],
            "timestamp_unix_ms" => &[dt1, dt2, dt3, dt4, dt5, dt6, dt7]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 7);
        // values:
        // 0: None
        // 1: 1.0 * 1100 = 1100
        // 2: 1.0 * 1200 = 1200
        // 3: -0.5 * 900 = -450
        // 4: 1.5 * 1500 = 2250
        // 5: 1.0 * 1600 = 1600
        // 6: -0.5 * 1000 = -500
        //
        // Period = 3
        // Seed at index 3: (1100 + 1200 - 450) / 3 = 1850 / 3 = 616.666
        // EMA at index 4: 2250 * 0.5 + 616.666 * 0.5 = 1125 + 308.333 = 1433.333
        // EMA at index 5: 1600 * 0.5 + 1433.333 * 0.5 = 800 + 716.666 = 1516.666
        // EMA at index 6: -500 * 0.5 + 1516.666 * 0.5 = -250 + 758.333 = 508.333

        assert!(out.get(3).is_some());
        assert!(out.get(4).is_some());
        assert!(out.get(5).is_some());
        assert!(out.get(6).is_some());

        Ok(())
    }
}
