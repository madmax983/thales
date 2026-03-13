use anyhow::{Context, Result};

use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Cast the timestamp column to chrono::DateTime<Utc> format per instructions
    let _dt_col = data.column("timestamp_unix_ms")
        .context("DataFrame must contain 'timestamp_unix_ms' column")?
        .cast(&DataType::Datetime(TimeUnit::Milliseconds, Some("UTC".to_string())))?;

    let close_series = data
        .column("close")
        .context("DataFrame must contain 'close' column")?;

    let sma_series = crate::indicators::sma::calculate(data, period)?;

    let shift_amount = (period / 2) + 1;
    let shifted_sma = sma_series.shift(shift_amount as i64);

    // To use Decimal for strictly accurate calculations, we must map over the paired Float64 series
    let close_f64 = close_series.f64()?;
    let sma_f64 = shifted_sma.f64()?;

    let mut dpo_series: Float64Chunked = close_f64
        .into_iter()
        .zip(sma_f64)
        .map(|(c_opt, s_opt)| {
            if let (Some(c), Some(s)) = (c_opt, s_opt) {
                if let (Some(c_dec), Some(s_dec)) = (
                    Decimal::from_f64_retain(c),
                    Decimal::from_f64_retain(s),
                ) {
                    let dpo_val = c_dec - s_dec;
                    Some(dpo_val.to_f64().unwrap_or(0.0))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    dpo_series.rename("dpo");

    Ok(dpo_series.into_series())
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[
                10.0, 11.0, 12.0, 13.0, 14.0,
                15.0, 14.0, 13.0, 12.0, 11.0,
                10.0, 11.0, 12.0, 13.0, 14.0
            ],
            "timestamp_unix_ms" => &[
                1672531200000i64, 1672617600000i64, 1672704000000i64, 1672790400000i64, 1672876800000i64,
                1672963200000i64, 1673049600000i64, 1673136000000i64, 1673222400000i64, 1673308800000i64,
                1673395200000i64, 1673481600000i64, 1673568000000i64, 1673654400000i64, 1673740800000i64
            ]
        )?;

        let period = 4;
        let res = calculate(&df, period)?;

        let out = res.cast(&DataType::Float64)?;
        let f64_out = out.f64()?;

        assert_eq!(f64_out.get(6), Some(2.5));
        assert_eq!(f64_out.get(7), Some(0.5));

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());

        let df_short = df!(
            "close" => &[10.0, 11.0],
            "timestamp_unix_ms" => &[1672531200000i64, 1672617600000i64]
        )?;

        let res_short = calculate(&df_short, 5)?;
        let out = res_short.cast(&DataType::Float64)?;
        let f64_out = out.f64()?;
        assert_eq!(f64_out.len(), 2);
        assert!(f64_out.get(0).is_none());
        assert!(f64_out.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0)
            .collect();

        let timestamps: Vec<i64> = (0..100).map(|i| 1672531200000i64 + (i as i64) * 60000).collect();

        let df = df!(
            "close" => values,
            "timestamp_unix_ms" => timestamps
        )?;

        let result = calculate(&df, 20);
        assert!(result.is_ok());

        if let Ok(s) = result {
            assert_eq!(s.len(), 100);
            let out = s.cast(&DataType::Float64)?;
            let s_f64 = out.f64()?;
            assert!(s_f64.get(29).is_none());
            assert!(s_f64.get(30).is_some());
            assert!(s_f64.get(99).is_some());
        }

        Ok(())
    }
}
