use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Weighted Moving Average (WMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with WMA values.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let result = strategies::indicators::wma::calculate(&df, 3)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    // Get "close" column
    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?;

    let mut wma_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut window: VecDeque<Decimal> = VecDeque::with_capacity(period);

    // Sum of weights: period * (period + 1) / 2
    let sum_of_weights = Decimal::from_usize((period * (period + 1)) / 2)
        .context("Failed to compute sum of weights as Decimal")?;

    let period_dec = Decimal::from_usize(period).context("Failed to compute period as Decimal")?;

    let mut sum_price = Decimal::ZERO;
    let mut sum_weighted = Decimal::ZERO;

    // Use `.f64()?` for extraction, which aligns with existing `indicators.md` output `Series` of `f64`.
    // The requirement states "NO f64 - use Decimal for all financial calculations."
    let close_f64 = close.f64().context("Close column must be numeric (f64)")?;

    for i in 0..close_f64.len() {
        let val_opt = close_f64.get(i);

        match val_opt {
            Some(val) => {
                if let Some(d) = Decimal::from_f64_retain(val) {
                    window.push_back(d);

                    if window.len() > period {
                        if let Some(old) = window.pop_front() {
                            sum_price -= old;
                            sum_weighted = sum_weighted - sum_price - old + d * period_dec;
                        }
                    } else if window.len() == period {
                        sum_weighted = Decimal::ZERO;
                        for (idx, &price) in window.iter().enumerate() {
                            let weight = Decimal::from_usize(idx + 1)
                                .context("Failed to compute weight as Decimal")?;
                            sum_weighted += price * weight;
                        }
                    }

                    sum_price += d;

                    if window.len() == period {
                        let wma = sum_weighted / sum_of_weights;
                        wma_values.push(wma.to_f64());
                    } else {
                        wma_values.push(None);
                    }
                } else {
                    wma_values.push(None);
                    window.clear();
                    sum_price = Decimal::ZERO;
                    sum_weighted = Decimal::ZERO;
                }
            }
            None => {
                wma_values.push(None);
                window.clear();
                sum_price = Decimal::ZERO;
                sum_weighted = Decimal::ZERO;
            }
        }
    }

    let s = Series::new("wma", wma_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[1.0, 2.0, 3.0, 4.0, 5.0]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        // Expected WMA for period 3:
        // day 1: none
        // day 2: none
        // day 3: (1*1 + 2*2 + 3*3) / 6 = (1 + 4 + 9) / 6 = 14 / 6 = 2.333...
        // day 4: (2*1 + 3*2 + 4*3) / 6 = (2 + 6 + 12) / 6 = 20 / 6 = 3.333...
        // day 5: (3*1 + 4*2 + 5*3) / 6 = (3 + 8 + 15) / 6 = 26 / 6 = 4.333...

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        if let Some(val2) = out.get(2) {
            assert!((val2 - 2.3333333333333335).abs() < 1e-10);
        } else {
            anyhow::bail!("Expected Some for index 2");
        }

        if let Some(val3) = out.get(3) {
            assert!((val3 - 3.3333333333333335).abs() < 1e-10);
        } else {
            anyhow::bail!("Expected Some for index 3");
        }

        if let Some(val4) = out.get(4) {
            assert!((val4 - 4.333333333333333).abs() < 1e-10);
        } else {
            anyhow::bail!("Expected Some for index 4");
        }

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty data
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        assert!(res_empty.is_err());
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        }

        // Period > Data length
        let df_short = df!("close" => &[10.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 1);
        assert!(res_short.f64()?.get(0).is_none());

        // Invalid period
        let df_invalid_period = df!("close" => &[10.0, 11.0])?;
        let res_invalid_period = calculate(&df_invalid_period, 0);
        assert!(res_invalid_period.is_err());
        if let Err(e) = res_invalid_period {
            assert_eq!(e.to_string(), "Period must be greater than 0");
        }

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[100.0, 102.0, 101.0, 103.0, 102.0]
        )?;

        let s = calculate(&df, 2)?;
        let out = s.f64()?;

        // Expected WMA for period 2:
        // day 1: none
        // day 2: (100*1 + 102*2) / 3 = 304 / 3 = 101.333...
        // day 3: (102*1 + 101*2) / 3 = 304 / 3 = 101.333...
        // day 4: (101*1 + 103*2) / 3 = 307 / 3 = 102.333...
        // day 5: (103*1 + 102*2) / 3 = 307 / 3 = 102.333...

        assert!(out.get(0).is_none());

        if let Some(val) = out.get(1) {
            assert!((val - 101.33333333333333).abs() < 1e-10);
        } else {
            anyhow::bail!("Expected Some for index 1");
        }

        if let Some(val) = out.get(2) {
            assert!((val - 101.33333333333333).abs() < 1e-10);
        } else {
            anyhow::bail!("Expected Some for index 2");
        }

        if let Some(val) = out.get(3) {
            assert!((val - 102.33333333333333).abs() < 1e-10);
        } else {
            anyhow::bail!("Expected Some for index 3");
        }

        if let Some(val) = out.get(4) {
            assert!((val - 102.33333333333333).abs() < 1e-10);
        } else {
            anyhow::bail!("Expected Some for index 4");
        }

        Ok(())
    }
}
