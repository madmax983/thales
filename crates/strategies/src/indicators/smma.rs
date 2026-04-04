use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Smoothed Moving Average (SMMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with SMMA values.
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
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let mut smma_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let mut prev_smma: Option<Decimal> = None;
    let mut window_sum = Decimal::ZERO;
    let mut count = 0;
    let period_dec = Decimal::from_usize(period).context("Invalid period")?;

    for i in 0..close.len() {
        let val_opt = close.get(i);

        match val_opt {
            Some(val) => {
                if let Some(d) = Decimal::from_f64_retain(val) {
                    if count < period {
                        window_sum += d;
                        count += 1;

                        if count == period {
                            // Calculate SMA as seed
                            let seed = window_sum / period_dec;
                            smma_values.push(seed.to_f64());
                            prev_smma = Some(seed);
                        } else {
                            smma_values.push(None);
                        }
                    } else {
                        // Calculate SMMA
                        if let Some(prev) = prev_smma {
                            let smma = (prev * (period_dec - Decimal::ONE) + d) / period_dec;
                            smma_values.push(smma.to_f64());
                            prev_smma = Some(smma);
                        } else {
                            // Should not happen if logic is correct
                            smma_values.push(None);
                        }
                    }
                } else {
                    // Invalid value (NaN, etc.)
                    // Reset
                    smma_values.push(None);
                    count = 0;
                    window_sum = Decimal::ZERO;
                    prev_smma = None;
                }
            }
            None => {
                // Missing value
                smma_values.push(None);
                count = 0;
                window_sum = Decimal::ZERO;
                prev_smma = None;
            }
        }
    }

    let s = Series::new("smma", smma_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smma_calculation() -> Result<()> {
        let mut df = df!("close" => &[10, 11, 12, 13, 14])?;
        df.try_apply("close", |s| s.cast(&DataType::Float64))?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert_eq!(out.get(2), Some(11.0));
        assert_eq!(out.get(3), Some(11.666666666666666));
        assert_eq!(out.get(4), Some(12.444444444444445));

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 5);
        if let Err(e) = res_empty {
            assert_eq!(e.to_string(), "Data cannot be empty");
        } else {
            anyhow::bail!("Expected error");
        }

        let mut df_short = df!("close" => &[10, 11])?;
        df_short.try_apply("close", |s| s.cast(&DataType::Float64))?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());
        assert!(res_short.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut df = df!(
            "close" => &[100, 102, 101, 103, 102]
        )?;
        df.try_apply("close", |s| s.cast(&DataType::Float64))?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert_eq!(out.get(1), Some(101.0));

        // SMMA_2 = (SMMA_1 * 1 + Close_2) / 2
        // SMMA_1 = 101
        // Close_2 (idx 2) = 101
        // SMMA_2 = (101 * 1 + 101) / 2 = 101
        assert_eq!(out.get(2), Some(101.0));

        // SMMA_3 = (SMMA_2 * 1 + Close_3) / 2
        // SMMA_2 = 101
        // Close_3 (idx 3) = 103
        // SMMA_3 = (101 * 1 + 103) / 2 = 102
        assert_eq!(out.get(3), Some(102.0));

        // SMMA_4 = (SMMA_3 * 1 + Close_4) / 2
        // SMMA_3 = 102
        // Close_4 (idx 4) = 102
        // SMMA_4 = (102 * 1 + 102) / 2 = 102
        assert_eq!(out.get(4), Some(102.0));

        Ok(())
    }
}
