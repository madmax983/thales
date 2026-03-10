//! Triple Exponential Moving Average (TEMA)
//!
//! Calculates the Triple Exponential Moving Average (TEMA) to reduce lag compared to traditional EMAs.
//! The formula is: TEMA = (3 * EMA1) - (3 * EMA2) + EMA3
//! where:
//! - EMA1 = EMA of price
//! - EMA2 = EMA of EMA1
//! - EMA3 = EMA of EMA2

use super::ema;
use anyhow::{Context, Result};
use polars::prelude::*;

/// Calculate Triple Exponential Moving Average (TEMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column.
/// * `period` - Lookback period.
///
/// # Returns
/// Series with TEMA values. The first values (up to `3 * period - 2`) will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let tema = strategies::indicators::tema::calculate(&df, 14)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // EMA1
    let ema1_series = ema::calculate(data, period).context("Failed to calculate EMA1")?;
    let ema1_df = DataFrame::new(vec![ema1_series.clone().with_name("close")])
        .context("Failed to create EMA1 DataFrame")?;

    // EMA2
    let ema2_series = ema::calculate(&ema1_df, period).context("Failed to calculate EMA2")?;
    let ema2_df = DataFrame::new(vec![ema2_series.clone().with_name("close")])
        .context("Failed to create EMA2 DataFrame")?;

    // EMA3
    let ema3_series = ema::calculate(&ema2_df, period).context("Failed to calculate EMA3")?;

    // Calculate TEMA: 3*EMA1 - 3*EMA2 + EMA3 using Polars vectorized operations
    let term1 = &ema1_series * 3.0;
    let term2 = &ema2_series * 3.0;
    let mut tema_series = (term1 - term2)?;
    tema_series = (tema_series + ema3_series)?;

    Ok(tema_series.with_name("tema").clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;
    use chrono::{Duration, Utc};

    #[test]
    fn test_tema_calculation() -> Result<()> {
        let mut closes = Vec::new();
        let mut timestamps = Vec::new();

        let now = Utc::now();
        for i in 1..=20 {
            closes.push(i as f64 * 10.0);
            timestamps.push(now + Duration::minutes(i as i64));
        }

        // Ensure timestamps are explicitly DateTime<Utc> in the dataframe series
        let time_series = DatetimeChunked::from_naive_datetime("timestamp", timestamps.into_iter().map(|t| t.naive_utc()).collect::<Vec<_>>(), TimeUnit::Milliseconds).into_series();

        let mut df = df!("close" => closes)?;
        df.with_column(time_series)?;

        let period = 3;
        let tema = calculate(&df, period)?;
        let t = tema.f64()?;

        assert_eq!(t.len(), 20);

        assert!(t.get(0).is_none());
        assert!(t.get(5).is_none());
        assert!(t.get(6).is_some());

        if let Some(val) = t.get(19) {
            assert!(val > 0.0);
        } else {
            anyhow::bail!("Value at index 19 should be Some");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res = calculate(&df_empty, 10);
        if let Err(e) = res {
            assert_eq!(e.to_string(), "Data cannot be empty");
        } else {
            anyhow::bail!("Expected error on empty DataFrame");
        }

        let closes = vec![10.0, 20.0];

        let timestamps = vec![
            Utc::now().naive_utc(),
            (Utc::now() + Duration::minutes(1)).naive_utc()
        ];
        let time_series = DatetimeChunked::from_naive_datetime("timestamp", timestamps, TimeUnit::Milliseconds).into_series();

        let mut df_valid = df!("close" => closes)?;
        df_valid.with_column(time_series)?;

        let res_zero_period = calculate(&df_valid, 0);
        if let Err(e) = res_zero_period {
            assert_eq!(e.to_string(), "Period must be greater than 0");
        } else {
            anyhow::bail!("Expected error on zero period");
        }

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let closes = vec![
            100.0, 101.0, 101.0, 102.0, 103.0, 102.0, 104.0, 105.0, 106.0, 105.0,
            107.0, 108.0, 108.0, 109.0, 110.0, 109.0, 111.0, 112.0, 113.0, 112.0,
            114.0, 115.0, 115.0, 116.0, 117.0, 116.0, 118.0, 119.0, 120.0, 119.0,
        ];

        let timestamps: Vec<_> = (0..30).map(|i| (Utc::now() + Duration::minutes(i)).naive_utc()).collect();
        let time_series = DatetimeChunked::from_naive_datetime("timestamp", timestamps, TimeUnit::Milliseconds).into_series();

        let mut df = df!("close" => closes)?;
        df.with_column(time_series)?;

        let period = 5;
        let tema = calculate(&df, period)?;

        assert_eq!(tema.len(), 30);

        let t = tema.f64()?;

        if let Some(val) = t.get(29) {
            assert!(!val.is_nan());
        } else {
            anyhow::bail!("Value at index 29 should be Some");
        }

        Ok(())
    }
}
