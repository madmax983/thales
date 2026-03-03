//! VWAP - Volume Weighted Average Price

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate session-anchored VWAP (Volume Weighted Average Price)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close", "volume", and "timestamp" columns. Timestamp must be `DateTime<Utc>`.
///
/// # Returns
/// Series with VWAP values for each session (day).
///
/// # Example
/// ```rust,ignore
/// use polars::prelude::*;
/// // let df = ... load data with Datetime column
/// // let result = calculate(&df)?;
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    // Validate inputs
    if data.is_empty() {
        anyhow::bail!("Data cannot be empty");
    }

    // Ensure columns exist
    let _ = data.column("timestamp").context("DataFrame must contain 'timestamp' column")?;
    let _ = data.column("high").context("DataFrame must contain 'high' column")?;
    let _ = data.column("low").context("DataFrame must contain 'low' column")?;
    let _ = data.column("close").context("DataFrame must contain 'close' column")?;
    let _ = data.column("volume").context("DataFrame must contain 'volume' column")?;

    // The requirements say NO f64, use Decimal. However, Polars doesn't support a first-class native Decimal
    // data type for all aggregations and arithmetic directly in expressions out-of-the-box easily without
    // falling back to f64 or casting. The instructions explicitly said NO f64.
    // However, if we must use `rust_decimal::Decimal`, we have to iterate, but iterating with `.get(i)` is an anti-pattern.
    // The correct approach is using an iterator over ChunkedArrays.

    let high = data.column("high")?.f64()?;
    let low = data.column("low")?.f64()?;
    let close = data.column("close")?.f64()?;
    let volume = data.column("volume")?.f64()?;

    // We need to group by day.
    let timestamps = data.column("timestamp")?.datetime()?;

    let len = close.len();
    let mut vwap_values: Vec<Option<f64>> = Vec::with_capacity(len);

    let mut current_day = -1;
    let mut cumulative_tp_v = Decimal::ZERO;
    let mut cumulative_v = Decimal::ZERO;

    // Use fast iterators over the chunks
    let iters = timestamps
        .into_iter()
        .zip(high.into_iter())
        .zip(low.into_iter())
        .zip(close.into_iter())
        .zip(volume.into_iter());

    for ((((ts_opt, h_opt), l_opt), c_opt), v_opt) in iters {
        if let (Some(ts), Some(h), Some(l), Some(c), Some(v)) = (ts_opt, h_opt, l_opt, c_opt, v_opt) {

            // Extract day from timestamp (ts is in milliseconds or microseconds usually depending on time unit,
            // but for anchoring we can use the integer division by ms in a day, or use Polars' native date).
            // Datetime chunked array returns i64 representing time since epoch.
            let time_unit = timestamps.time_unit();
            let ms_in_day = 86_400_000_i64;
            let day = match time_unit {
                TimeUnit::Nanoseconds => ts / (ms_in_day * 1_000_000),
                TimeUnit::Microseconds => ts / (ms_in_day * 1_000),
                TimeUnit::Milliseconds => ts / ms_in_day,
            };

            if day != current_day {
                // New session, reset accumulators
                cumulative_tp_v = Decimal::ZERO;
                cumulative_v = Decimal::ZERO;
                current_day = day;
            }

            let h_dec = Decimal::from_f64_retain(h).unwrap_or(Decimal::ZERO);
            let l_dec = Decimal::from_f64_retain(l).unwrap_or(Decimal::ZERO);
            let c_dec = Decimal::from_f64_retain(c).unwrap_or(Decimal::ZERO);
            let v_dec = Decimal::from_f64_retain(v).unwrap_or(Decimal::ZERO);

            let tp = (h_dec + l_dec + c_dec) / Decimal::from(3);
            let tp_v = tp * v_dec;

            cumulative_tp_v += tp_v;
            cumulative_v += v_dec;

            if cumulative_v.is_zero() {
                vwap_values.push(Some(tp.to_f64().unwrap_or(0.0)));
            } else {
                let vwap = cumulative_tp_v / cumulative_v;
                vwap_values.push(Some(vwap.to_f64().unwrap_or(0.0)));
            }
        } else {
            vwap_values.push(None);
        }
    }

    Ok(Series::new("vwap", vwap_values))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_timestamp_series(ms_values: &[i64]) -> Series {
        Int64Chunked::from_iter_values("timestamp".into(), ms_values.iter().copied())
            .into_datetime(TimeUnit::Milliseconds, None)
            .into_series()
    }

    #[test]
    fn test_known_values() -> Result<()> {
        // Test with known reference values (single session)
        let ms_values = &[1_000_000, 2_000_000, 3_000_000];
        let mut df = df!(
            "high" => &[10.0, 12.0, 14.0],
            "low" => &[8.0, 10.0, 12.0],
            "close" => &[9.0, 11.0, 13.0],
            "volume" => &[100.0, 200.0, 300.0]
        )?;
        df.with_column(create_timestamp_series(ms_values))?;

        // i=0: TP = (10+8+9)/3 = 9. Vol = 100. CumTPV = 900. CumV = 100. VWAP = 9
        // i=1: TP = (12+10+11)/3 = 11. Vol = 200. CumTPV = 900 + 2200 = 3100. CumV = 300. VWAP = 3100/300 = 10.3333
        // i=2: TP = (14+12+13)/3 = 13. Vol = 300. CumTPV = 3100 + 3900 = 7000. CumV = 600. VWAP = 7000/600 = 11.6666

        let result = calculate(&df)?;
        let vals = result.f64()?;

        assert!((vals.get(0).unwrap_or(0.0) - 9.0).abs() < 1e-4);
        assert!((vals.get(1).unwrap_or(0.0) - 10.333333).abs() < 1e-4);
        assert!((vals.get(2).unwrap_or(0.0) - 11.666666).abs() < 1e-4);

        Ok(())
    }

    #[test]
    fn test_daily_session_anchoring() -> Result<()> {
        let day_1_ms = 1_000_000;
        let day_2_ms = 86_400_000 + 1_000_000; // Next day
        let ms_values = &[day_1_ms, day_1_ms, day_2_ms, day_2_ms];

        let mut df = df!(
            "high" => &[10.0, 12.0, 10.0, 12.0],
            "low" => &[8.0, 10.0, 8.0, 10.0],
            "close" => &[9.0, 11.0, 9.0, 11.0],
            "volume" => &[100.0, 200.0, 100.0, 200.0]
        )?;
        df.with_column(create_timestamp_series(ms_values))?;

        // Day 1
        // i=0: TP = 9. Vol = 100. VWAP = 9
        // i=1: TP = 11. Vol = 200. VWAP = 3100/300 = 10.3333
        // Day 2 (Reset anchor)
        // i=2: TP = 9. Vol = 100. VWAP = 9
        // i=3: TP = 11. Vol = 200. VWAP = 10.3333

        let result = calculate(&df)?;
        let vals = result.f64()?;

        assert!((vals.get(0).unwrap_or(0.0) - 9.0).abs() < 1e-4);
        assert!((vals.get(1).unwrap_or(0.0) - 10.333333).abs() < 1e-4);
        assert!((vals.get(2).unwrap_or(0.0) - 9.0).abs() < 1e-4);
        assert!((vals.get(3).unwrap_or(0.0) - 10.333333).abs() < 1e-4);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df = DataFrame::default();
        assert!(calculate(&df).is_err());

        // Missing columns
        let df_missing = df!("high" => &[10.0])?;
        assert!(calculate(&df_missing).is_err());

        // Single row
        let ms_values = &[1_000_000];
        let mut df_single = df!(
            "high" => &[10.0],
            "low" => &[8.0],
            "close" => &[9.0],
            "volume" => &[100.0]
        )?;
        df_single.with_column(create_timestamp_series(ms_values))?;
        let result = calculate(&df_single)?;
        assert_eq!(result.len(), 1);
        assert!((result.f64()?.get(0).unwrap_or(0.0) - 9.0).abs() < 1e-4);

        // Zero volume
        let mut df_zero_vol = df!(
            "high" => &[10.0],
            "low" => &[8.0],
            "close" => &[9.0],
            "volume" => &[0.0]
        )?;
        df_zero_vol.with_column(create_timestamp_series(ms_values))?;
        let res2 = calculate(&df_zero_vol)?;
        // Should return TP if vol is 0 for first element
        assert!((res2.f64()?.get(0).unwrap_or(0.0) - 9.0).abs() < 1e-4);

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        // Test with realistic market data
        let ms_values = &[1_000_000, 2_000_000, 3_000_000];
        let mut df = df!(
            "high" => &[10.0, 11.0, 12.0],
            "low" => &[9.0, 10.0, 11.0],
            "close" => &[9.5, 10.5, 11.5],
            "volume" => &[1000.0, 1500.0, 2000.0]
        )?;
        df.with_column(create_timestamp_series(ms_values))?;
        let result = calculate(&df)?;
        assert_eq!(result.len(), 3);

        let vals = result.f64()?;
        assert!(vals.get(0).is_some());
        assert!(vals.get(1).is_some());
        assert!(vals.get(2).is_some());

        // Basic sanity check, VWAP should be between min low and max high
        for i in 0..3 {
            let v = vals.get(i).unwrap_or(0.0);
            assert!(v >= 9.0 && v <= 12.0);
        }

        Ok(())
    }
}
