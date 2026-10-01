use anyhow::{Context, Result};
use polars::prelude::*;

/// Calculate Exponential Moving Average (EMA)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period
///
/// # Returns
/// Series with EMA values.
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

    let mut ema_values: Vec<Option<f64>> = Vec::with_capacity(close.len());
    let k = 2.0 / (period as f64 + 1.0);
    let mut prev_ema: Option<f64> = None;
    let mut window_sum = 0.0f64;
    let mut count = 0;
    let period_f = period as f64;

    for i in 0..close.len() {
        match close.get(i) {
            Some(d) if d.is_finite() => {
                if count < period {
                    window_sum += d;
                    count += 1;
                    if count == period {
                        let seed = window_sum / period_f;
                        ema_values.push(Some(seed));
                        prev_ema = Some(seed);
                    } else {
                        ema_values.push(None);
                    }
                } else if let Some(prev) = prev_ema {
                    let ema = d * k + prev * (1.0 - k);
                    ema_values.push(Some(ema));
                    prev_ema = Some(ema);
                } else {
                    ema_values.push(None);
                }
            }
            _ => {
                ema_values.push(None);
                count = 0;
                window_sum = 0.0;
                prev_ema = None;
            }
        }
    }

    let s = Series::new("ema", ema_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_ema_calculation() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 11.0, 12.0, 13.0, 14.0]
        )?;

        // Period 3. K = 2/4 = 0.5
        // 0: 10.0 -> None (sum 10)
        // 1: 11.0 -> None (sum 21)
        // 2: 12.0 -> SMA(10,11,12) = 11.0. Seed.
        // 3: 13.0 -> (13 * 0.5) + (11 * 0.5) = 6.5 + 5.5 = 12.0
        // 4: 14.0 -> (14 * 0.5) + (12 * 0.5) = 7 + 6 = 13.0

        let result = calculate(&df, 3)?;
        let out = result.f64()?;

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert_eq!(out.get(2), Some(11.0));
        assert_eq!(out.get(3), Some(12.0));
        assert_eq!(out.get(4), Some(13.0));

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
}
