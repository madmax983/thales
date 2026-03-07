use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Center of Gravity (CG) oscillator
///
/// The Center of Gravity indicator is an oscillator developed by John Ehlers.
/// It helps identify turning points as early as possible with zero lag.
///
/// Formula:
/// CG = Sum(Price_i * (i + 1)) / Sum(Price_i)
/// where i ranges from 0 to period-1 (0 is current bar, period-1 is oldest).
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with CG values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// use strategies::indicators::cg;
///
/// let df = df!("close" => &[10.0, 20.0, 30.0, 40.0, 50.0]).unwrap();
/// let cg_series = cg::calculate(&df, 3).unwrap();
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let close_series = data
        .column("close")
        .context("DataFrame must contain 'close' column")?;

    let close_cast = close_series
        .cast(&DataType::Float64)
        .context("Failed to cast close column to Float64")?;

    let close = close_cast
        .f64()
        .context("Close column must be numeric (f64)")?;

    let mut cg_values: Vec<Option<f64>> = Vec::with_capacity(close.len());

    // Pre-extract data into a Vec to avoid slow repeated ChunkedArray::get() calls
    let close_prices: Vec<Option<Decimal>> = close
        .into_iter()
        .map(|opt_val| {
            opt_val.and_then(Decimal::from_f64_retain)
        })
        .collect();

    for i in 0..close_prices.len() {
        if i < period - 1 {
            cg_values.push(None);
            continue;
        }

        let mut num_sum = Decimal::ZERO;
        let mut den_sum = Decimal::ZERO;
        let mut valid = true;

        for j in 0..period {
            let idx = i.saturating_sub(j);

            if let Some(Some(price)) = close_prices.get(idx) {
                let weight = Decimal::from(j + 1);
                num_sum += price * weight;
                den_sum += price;
            } else {
                valid = false;
                break;
            }
        }

        if valid && !den_sum.is_zero() {
            let cg = num_sum / den_sum;
            cg_values.push(cg.to_f64());
        } else {
            cg_values.push(None);
        }
    }

    Ok(Series::new("cg", cg_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_cg_calculation() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 20.0, 30.0, 40.0, 50.0]
        )?;

        // Period = 3
        // i=0: None
        // i=1: None
        // i=2: [10, 20, 30] -> Current is 30
        // Num: 30*1 + 20*2 + 10*3 = 30 + 40 + 30 = 100
        // Den: 30 + 20 + 10 = 60
        // CG: 100 / 60 = 1.6666...
        // i=3: [20, 30, 40] -> Current is 40
        // Num: 40*1 + 30*2 + 20*3 = 40 + 60 + 60 = 160
        // Den: 40 + 30 + 20 = 90
        // CG: 160 / 90 = 1.777...
        // i=4: [30, 40, 50] -> Current is 50
        // Num: 50*1 + 40*2 + 30*3 = 50 + 80 + 90 = 220
        // Den: 50 + 40 + 30 = 120
        // CG: 220 / 120 = 1.8333...

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        let cg2 = out.get(2).unwrap();
        assert!((cg2 - 1.6666666666666667).abs() < 1e-10);

        let cg3 = out.get(3).unwrap();
        assert!((cg3 - 1.7777777777777777).abs() < 1e-10);

        let cg4 = out.get(4).unwrap();
        assert!((cg4 - 1.8333333333333333).abs() < 1e-10);

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

        // Zero period
        let df_normal = df!("close" => &[10.0, 20.0, 30.0])?;
        let res_zero = calculate(&df_normal, 0);
        assert!(res_zero.is_err());
        assert_eq!(res_zero.unwrap_err().to_string(), "Period must be greater than 0");

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        // Ensure no panics on larger sequences
        let mut closes = Vec::new();
        for i in 0..100 {
            closes.push(100.0 + (i as f64).sin() * 10.0);
        }

        let df = df!("close" => &closes)?;
        let res = calculate(&df, 10)?;
        assert_eq!(res.len(), 100);
        let out = res.f64()?;
        assert!(out.get(0).is_none());
        assert!(out.get(8).is_none());
        assert!(out.get(9).is_some());
        assert!(out.get(99).is_some());

        Ok(())
    }
}
