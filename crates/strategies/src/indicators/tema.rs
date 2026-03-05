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

    // Convert EMA1 series to a DataFrame to pass to ema::calculate
    let ema1_df = DataFrame::new(vec![ema1_series.clone().with_name("close")])
        .context("Failed to create EMA1 DataFrame")?;

    // EMA2
    let ema2_series = ema::calculate(&ema1_df, period).context("Failed to calculate EMA2")?;

    // Convert EMA2 series to a DataFrame to pass to ema::calculate
    let ema2_df = DataFrame::new(vec![ema2_series.clone().with_name("close")])
        .context("Failed to create EMA2 DataFrame")?;

    // EMA3
    let ema3_series = ema::calculate(&ema2_df, period).context("Failed to calculate EMA3")?;

    // Calculate TEMA: 3*EMA1 - 3*EMA2 + EMA3
    let ema1 = ema1_series.f64()?;
    let ema2 = ema2_series.f64()?;
    let ema3 = ema3_series.f64()?;

    let len = data.height();
    let mut tema_values = Vec::with_capacity(len);

    for i in 0..len {
        let val1 = ema1.get(i);
        let val2 = ema2.get(i);
        let val3 = ema3.get(i);

        if let (Some(v1), Some(v2), Some(v3)) = (val1, val2, val3) {
            let tema = (3.0 * v1) - (3.0 * v2) + v3;
            tema_values.push(Some(tema));
        } else {
            tema_values.push(None);
        }
    }

    Ok(Series::new("tema", tema_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_tema_calculation() -> Result<()> {
        // Linear data to test TEMA behavior
        let closes: Vec<f64> = (1..=20).map(|i| i as f64 * 10.0).collect();
        let df = df!("close" => closes)?;

        let period = 3;
        let tema = calculate(&df, period)?;
        let t = tema.f64()?;

        assert_eq!(t.len(), 20);

        // First few should be None due to EMA warmups
        // EMA1 takes `period - 1` = 2
        // EMA2 takes `period - 1` on top of EMA1 = 4
        // EMA3 takes `period - 1` on top of EMA2 = 6
        // Actually EMA implementation sets first `period - 1` to None.
        assert!(t.get(0).is_none());
        assert!(t.get(5).is_none());
        assert!(t.get(6).is_some()); // Index 6 is the 7th element

        // Verify values are somewhat reasonable (not NaN)
        assert!(t.get(19).unwrap() > 0.0);

        Ok(())
    }

    #[test]
    fn test_edge_cases() {
        let df_empty = DataFrame::default();
        let res = calculate(&df_empty, 10);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Data cannot be empty");

        let df_valid = df!("close" => &[10.0, 20.0]).unwrap();
        let res_zero_period = calculate(&df_valid, 0);
        assert!(res_zero_period.is_err());
        assert_eq!(res_zero_period.unwrap_err().to_string(), "Period must be greater than 0");
    }
}
