use anyhow::{Context, Result};
use chrono::DateTime;
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Volume Weighted Average Price (VWAP)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close", "volume", and "timestamp_unix_ms" columns
///
/// # Returns
/// Series with VWAP values, reset daily based on the timestamp.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ... load data
/// // let result = calculate(&df)?;
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
        .cast(&DataType::Int64)?
        .i64()?
        .clone();

    let mut vwap_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut current_day: Option<chrono::NaiveDate> = None;

    let mut cumulative_price_volume = Decimal::ZERO;
    let mut cumulative_volume = Decimal::ZERO;

    for i in 0..close.len() {
        let ts_ms = timestamp.get(i).unwrap_or(0);

        let datetime = DateTime::from_timestamp(ts_ms / 1000, (ts_ms % 1000) as u32 * 1_000_000)
            .unwrap_or_default();
        let date = datetime.date_naive();

        let h_opt = high.get(i);
        let l_opt = low.get(i);
        let c_opt = close.get(i);
        let v_opt = volume.get(i);

        match (h_opt, l_opt, c_opt, v_opt) {
            (Some(h), Some(l), Some(c), Some(v)) => {
                // Reset cumulative values if it's a new day
                if let Some(prev_date) = current_day {
                    if date != prev_date {
                        cumulative_price_volume = Decimal::ZERO;
                        cumulative_volume = Decimal::ZERO;
                    }
                }
                current_day = Some(date);

                let h_dec = Decimal::from_f64_retain(h).unwrap_or(Decimal::ZERO);
                let l_dec = Decimal::from_f64_retain(l).unwrap_or(Decimal::ZERO);
                let c_dec = Decimal::from_f64_retain(c).unwrap_or(Decimal::ZERO);
                let v_dec = Decimal::from_f64_retain(v).unwrap_or(Decimal::ZERO);

                let typical_price = (h_dec + l_dec + c_dec) / Decimal::from(3);

                cumulative_price_volume += typical_price * v_dec;
                cumulative_volume += v_dec;

                if cumulative_volume.is_zero() {
                    vwap_values.push(Some(typical_price.to_f64().unwrap_or(0.0)));
                } else {
                    let vwap = cumulative_price_volume
                        .checked_div(cumulative_volume)
                        .unwrap_or(Decimal::ZERO);
                    vwap_values.push(Some(vwap.to_f64().unwrap_or(0.0)));
                }
            }
            _ => {
                vwap_values.push(None);
            }
        }
    }

    Ok(Series::new("vwap", vwap_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "timestamp_unix_ms" => &[
                1672531200000i64, // 2023-01-01 00:00:00 UTC
                1672534800000i64, // 2023-01-01 01:00:00 UTC
                1672617600000i64, // 2023-01-02 00:00:00 UTC (Next Day!)
                1672621200000i64, // 2023-01-02 01:00:00 UTC
            ],
            "high" => &[10.0, 12.0, 20.0, 22.0],
            "low" => &[8.0, 10.0, 18.0, 20.0],
            "close" => &[9.0, 11.0, 19.0, 21.0],
            "volume" => &[100.0, 200.0, 300.0, 400.0]
        )?;

        // VWAP Calculation logic:
        // Typical Price (TP) = (High + Low + Close) / 3
        // Cumulative TP * Volume / Cumulative Volume, reset each day.

        let result = calculate(&df)?;
        let out = result.f64()?;

        // Day 1:
        // i=0: TP = (10+8+9)/3 = 9. Vol = 100. VWAP = 9 * 100 / 100 = 9.0.
        // i=1: TP = (12+10+11)/3 = 11. Vol = 200. VWAP = (9*100 + 11*200) / (100+200) = 3100 / 300 = 10.33333333...
        // Day 2 (Reset):
        // i=2: TP = (20+18+19)/3 = 19. Vol = 300. VWAP = 19.0
        // i=3: TP = (22+20+21)/3 = 21. Vol = 400. VWAP = (19*300 + 21*400) / (300+400) = 14100 / 700 = 20.14285714...

        assert_eq!(out.get(0), Some(9.0));

        let val1 = out.get(1).unwrap_or(0.0);
        assert!((val1 - 10.33333333).abs() < 1e-5);

        assert_eq!(out.get(2), Some(19.0));

        let val3 = out.get(3).unwrap_or(0.0);
        assert!((val3 - 20.14285714).abs() < 1e-5);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty).is_err());

        let df_short = df!(
            "timestamp_unix_ms" => &[1672531200000i64],
            "high" => &[10.0],
            "low" => &[10.0],
            "close" => &[10.0],
            "volume" => &[100.0]
        )?;
        let res = calculate(&df_short)?;
        let out = res.f64()?;
        assert_eq!(out.get(0), Some(10.0));
        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "timestamp_unix_ms" => &[
                1672531200000i64, // 2023-01-01 00:00:00 UTC
                1672531260000i64, // 2023-01-01 00:01:00 UTC
            ],
            "high" => &[10.0, 10.0],
            "low" => &[10.0, 10.0],
            "close" => &[10.0, 10.0],
            "volume" => &[0.0, 100.0] // Zero volume edge case
        )?;

        let result = calculate(&df)?;
        let out = result.f64()?;

        // i=0: Vol = 0. VWAP = TP (fallback) = 10.0
        // i=1: Vol = 100. VWAP = (0 + 10*100) / 100 = 10.0
        assert_eq!(out.get(0), Some(10.0));
        assert_eq!(out.get(1), Some(10.0));

        Ok(())
    }
}
