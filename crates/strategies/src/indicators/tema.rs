//! TEMA - Triple Exponential Moving Average
//!
//! TEMA was developed by Patrick Mulloy and attempts to remove the inherent lag
//! associated with Moving Averages by placing more weight on recent data.
//! It is calculated as: (3 * EMA1) - (3 * EMA2) + EMA3
//! where EMA1 is the EMA of the price, EMA2 is the EMA of EMA1, and EMA3 is the EMA of EMA2.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;

use super::ema;

/// Calculate Triple Exponential Moving Average (TEMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with TEMA values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::tema;
/// // let df = ... load data
/// // let result = tema::calculate(&df, 14).unwrap();
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // 1. Calculate EMA1
    let ema1_series = ema::calculate(data, period).context("Failed to calculate EMA1")?;

    // Create a DataFrame for EMA1 to pass to the ema calculation
    let ema1_df = DataFrame::new(vec![ema1_series.clone().with_name("close")])
        .context("Failed to create DataFrame for EMA1")?;

    // 2. Calculate EMA2 (EMA of EMA1)
    let ema2_series = ema::calculate(&ema1_df, period).context("Failed to calculate EMA2")?;

    // Create a DataFrame for EMA2 to pass to the ema calculation
    let ema2_df = DataFrame::new(vec![ema2_series.clone().with_name("close")])
        .context("Failed to create DataFrame for EMA2")?;

    // 3. Calculate EMA3 (EMA of EMA2)
    let ema3_series = ema::calculate(&ema2_df, period).context("Failed to calculate EMA3")?;

    let ema1_chunked = ema1_series.f64().context("EMA1 should be f64")?;
    let ema2_chunked = ema2_series.f64().context("EMA2 should be f64")?;
    let ema3_chunked = ema3_series.f64().context("EMA3 should be f64")?;

    let mut tema_values: Vec<Option<f64>> = Vec::with_capacity(data.height());

    let three = Decimal::new(3, 0);

    for i in 0..data.height() {
        let v1 = ema1_chunked.get(i);
        let v2 = ema2_chunked.get(i);
        let v3 = ema3_chunked.get(i);

        if let (Some(e1), Some(e2), Some(e3)) = (v1, v2, v3) {
            let d1 = Decimal::from_f64_retain(e1);
            let d2 = Decimal::from_f64_retain(e2);
            let d3 = Decimal::from_f64_retain(e3);

            if let (Some(d1), Some(d2), Some(d3)) = (d1, d2, d3) {
                // TEMA = (3 * EMA1) - (3 * EMA2) + EMA3
                let tema = (three * d1) - (three * d2) + d3;
                tema_values.push(tema.to_f64());
            } else {
                tema_values.push(None);
            }
        } else {
            tema_values.push(None);
        }
    }

    let s = Series::new("tema", tema_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_tema_calculation() -> Result<()> {
        // TEMA requires 3x the period of data to fully warm up, but we'll test a small slice.
        // We'll use values where we can trace the math easily or rely on the underlying ema logic.
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 9);

        // Let's trace period 2 logic carefully based on the `ema` indicator
        // EMA period 2 requires 2 values for the seed.
        // Data: [10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0]
        // EMA1(2):
        //  0: 10.0 -> None
        //  1: 11.0 -> 10.5 (Seed)
        //  2: 12.0 -> ...
        // EMA1 has valid data starting at index 1.
        // EMA2(2) input = [None, 10.5, ...]:
        //  0: None -> None
        //  1: 10.5 -> None
        //  2: ... -> Seed at index 2
        // EMA2 has valid data starting at index 2.
        // EMA3(2) input = [None, None, Seed, ...]:
        //  0: None -> None
        //  1: None -> None
        //  2: Seed -> None
        //  3: ... -> Seed at index 3
        // EMA3 has valid data starting at index 3.

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert!(out.get(2).is_none());
        assert!(out.get(3).is_some());
        assert!(out.get(4).is_some());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        assert_eq!(res_empty.unwrap_err().to_string(), "Data cannot be empty");

        // Period > Data length
        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        // A simple up-trend
        let mut close_vals = Vec::new();
        for i in 1..=50 {
            close_vals.push(i as f64);
        }

        let df = DataFrame::new(vec![Series::new("close", close_vals)])?;
        let result = calculate(&df, 10)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 50);

        // TEMA responds faster than EMA, so in a steady uptrend, TEMA should be closer to price
        if let Some(last_tema) = out.get(49) {
            // Since we are at 50, and TEMA follows closely, it should be quite high (near 50)
            assert!(last_tema > 45.0);
        } else {
            anyhow::bail!("Expected TEMA value at index 49 to be Some, got None");
        }

        Ok(())
    }
}
