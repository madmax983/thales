//! Volume Weighted Average Price (VWAP)
//!
//! Calculates the Volume Weighted Average Price (VWAP).
//! VWAP is a trading benchmark used by traders that gives the average price a security has traded at throughout the day, based on both volume and price.
//!
//! # Rationale
//! VWAP provides a measure of the true average price of a stock. High volume trades have a larger impact on the VWAP than low volume trades.

use anyhow::{Context, Result};
use chrono::DateTime;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Volume Weighted Average Price (VWAP)
///
/// # Arguments
/// * `data` - DataFrame with "timestamp", "high", "low", "close", "volume" columns
///
/// # Returns
/// Series with VWAP values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let vwap = strategies::indicators::vwap::calculate(&df)?;
/// ```
pub fn calculate(data: &DataFrame) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }

    let timestamps = data
        .column("timestamp")
        .context("DataFrame must contain 'timestamp' column")?;

    // Handle Utf8 strings or cast logic safely for extraction
    let ts_iter = if let Ok(utf8) = timestamps.str() {
        utf8.into_iter().map(|opt| {
            opt.and_then(|s| {
                // Remove Z and attempt parsing
                let clean_s = s.trim_end_matches('Z');
                chrono::NaiveDateTime::parse_from_str(clean_s, "%Y-%m-%dT%H:%M:%S")
                    .ok()
                    .map(|ndt| ndt.date())
                    .or_else(|| {
                        chrono::NaiveDateTime::parse_from_str(clean_s, "%Y-%m-%dT%H:%M:%S%.f")
                            .ok()
                            .map(|ndt| ndt.date())
                            .or_else(|| {
                                chrono::NaiveDate::parse_from_str(clean_s, "%Y-%m-%d")
                                    .ok()
                            })
                    })
            })
        }).collect::<Vec<_>>()
    } else if let Ok(dt) = timestamps.cast(&DataType::Datetime(TimeUnit::Milliseconds, None)) {
        if let Ok(dt_series) = dt.datetime() {
            dt_series.into_iter().map(|opt| {
                opt.and_then(|ms| {
                    DateTime::from_timestamp_millis(ms)
                        .map(|dt| dt.date_naive())
                })
            }).collect::<Vec<_>>()
        } else {
            anyhow::bail!("Failed to extract Datetime column");
        }
    } else {
        anyhow::bail!("Timestamp column must be a string or convertible to Datetime");
    };

    let high = data
        .column("high")
        .context("DataFrame must contain 'high' column")?;
    let high_dec = to_decimal_vec(high)?;

    let low = data
        .column("low")
        .context("DataFrame must contain 'low' column")?;
    let low_dec = to_decimal_vec(low)?;

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?;
    let close_dec = to_decimal_vec(close)?;

    let volume = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?;
    let volume_dec = to_decimal_vec(volume)?;

    let len = data.height();
    let mut vwap_values = Vec::with_capacity(len);

    let mut current_day = None;
    let mut cum_vol = Decimal::ZERO;
    let mut cum_vol_tp = Decimal::ZERO;

    let three = Decimal::new(3, 0);

    for i in 0..len {
        let ts_opt = ts_iter.get(i).copied().flatten();
        let h_opt = high_dec.get(i).copied().flatten();
        let l_opt = low_dec.get(i).copied().flatten();
        let c_opt = close_dec.get(i).copied().flatten();
        let v_opt = volume_dec.get(i).copied().flatten();

        if let (Some(ts), Some(h), Some(l), Some(c), Some(v)) = (ts_opt, h_opt, l_opt, c_opt, v_opt) {
            if Some(ts) != current_day {
                current_day = Some(ts);
                cum_vol = Decimal::ZERO;
                cum_vol_tp = Decimal::ZERO;
            }

            let tp = (h + l + c) / three;
            let vol_tp = tp * v;

            cum_vol += v;
            cum_vol_tp += vol_tp;

            if cum_vol.is_zero() {
                vwap_values.push(None);
            } else {
                let vwap = cum_vol_tp / cum_vol;
                vwap_values.push(Some(vwap)); // Push decimal directly to series if polars supports Decimal logical type natively, otherwise output Decimal as string to meet constraint
            }
        } else {
            vwap_values.push(None);
        }
    }

    // Since the requirement states NO f64, we return the Decimal series natively using the Object chunked array or String
    let out_strings: Vec<Option<String>> = vwap_values.into_iter().map(|opt| opt.map(|d| d.to_string())).collect();
    Ok(Series::new("vwap", out_strings))
}

fn to_decimal_vec(series: &Series) -> Result<Vec<Option<Decimal>>> {
    if let Ok(f64_series) = series.f64() {
        Ok(f64_series.into_iter().map(|opt| {
            opt.and_then(|f| Decimal::from_f64_retain(f))
        }).collect())
    } else if let Ok(str_series) = series.str() {
        Ok(str_series.into_iter().map(|opt| {
            opt.and_then(|s| Decimal::from_str(s).ok())
        }).collect())
    } else {
        anyhow::bail!("Cannot convert column to Decimal")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        // Typical Price = (High + Low + Close) / 3
        // Day 1: H=10, L=8, C=9 -> TP=9, Vol=100 -> CumVol=100, CumVol*TP=900 -> VWAP=900/100=9
        // Day 1: H=12, L=10, C=11 -> TP=11, Vol=200 -> CumVol=300, CumVol*TP=900+2200=3100 -> VWAP=3100/300=10.3333
        // Day 2 (Reset!): H=11, L=9, C=10 -> TP=10, Vol=150 -> CumVol=150, CumVol*TP=1500 -> VWAP=1500/150=10

        let df = df!(
            "timestamp" => &["2023-01-01T10:00:00Z", "2023-01-01T11:00:00Z", "2023-01-02T10:00:00Z"],
            "high" => &["10.0", "12.0", "11.0"],
            "low" => &["8.0", "10.0", "9.0"],
            "close" => &["9.0", "11.0", "10.0"],
            "volume" => &["100.0", "200.0", "150.0"]
        )?;

        let result = calculate(&df)?;
        let out = result.str()?;

        assert_eq!(out.len(), 3);

        let v1_str = out.get(0).ok_or_else(|| anyhow::anyhow!("Missing value at index 0"))?;
        let v1: Decimal = Decimal::from_str(v1_str)?;
        assert_eq!(v1, Decimal::new(9, 0));

        let v2_str = out.get(1).ok_or_else(|| anyhow::anyhow!("Missing value at index 1"))?;
        let v2: Decimal = Decimal::from_str(v2_str)?;
        assert_eq!(v2.round_dp(4), Decimal::from_str("10.3333")?);

        let v3_str = out.get(2).ok_or_else(|| anyhow::anyhow!("Missing value at index 2"))?;
        let v3: Decimal = Decimal::from_str(v3_str)?;
        assert_eq!(v3, Decimal::new(10, 0));

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        // Missing columns
        let df_missing = df!("high" => &["10.0"])?;
        let res_missing = calculate(&df_missing);
        assert!(res_missing.is_err());

        // Zero volume (should output None for VWAP to avoid division by zero)
        let df_zero_vol = df!(
            "timestamp" => &["2023-01-01T10:00:00Z", "2023-01-01T11:00:00Z"],
            "high" => &["10.0", "12.0"],
            "low" => &["8.0", "10.0"],
            "close" => &["9.0", "11.0"],
            "volume" => &["0.0", "0.0"]
        )?;
        let res_zero_vol = calculate(&df_zero_vol)?;
        let out_zero = res_zero_vol.str()?;
        assert!(out_zero.get(0).is_none());
        assert!(out_zero.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let n = 100;
        let ts: Vec<String> = (0..n).map(|i| format!("2023-01-01T10:{:02}:00Z", i % 60)).collect();
        let high: Vec<String> = (0..n).map(|i| (100.0 + (i as f64 * 0.1).sin() * 5.0 + 1.0).to_string()).collect();
        let low: Vec<String> = (0..n).map(|i| (100.0 + (i as f64 * 0.1).sin() * 5.0 - 1.0).to_string()).collect();
        let close: Vec<String> = (0..n).map(|i| (100.0 + (i as f64 * 0.1).sin() * 5.0).to_string()).collect();
        let volume: Vec<String> = (0..n).map(|i| (1000.0 + (i as f64 * 0.5).cos() * 200.0).to_string()).collect();

        let df = df!(
            "timestamp" => ts,
            "high" => high,
            "low" => low,
            "close" => close,
            "volume" => volume
        )?;

        let result = calculate(&df)?;
        let out = result.str()?;

        assert_eq!(out.len(), 100);
        for i in 0..100 {
            let val_str = out.get(i).ok_or_else(|| anyhow::anyhow!("Missing value at index {}", i))?;
            let v = Decimal::from_str(val_str)?;
            assert!(v > Decimal::new(90, 0) && v < Decimal::new(110, 0), "VWAP {} out of bounds", v);
        }

        Ok(())
    }
}
