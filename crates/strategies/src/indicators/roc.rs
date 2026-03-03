use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Rate of Change (ROC)
///
/// The Rate of Change (ROC) is a momentum-based technical indicator that measures
/// the percentage change in price between the current price and the price a certain
/// number of periods ago.
///
/// ROC = ((Close - Close(n periods ago)) / Close(n periods ago)) * 100
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period (n)
///
/// # Returns
/// Series with ROC values. The first `period` values will be null.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let mut roc_values: Vec<Option<f64>> = vec![None; close.len()];

    let hundred = Decimal::from(100);

    for (i, val) in roc_values.iter_mut().enumerate().take(close.len()).skip(period) {
        let current_val = close.get(i);
        let past_val = close.get(i - period);

        if let (Some(c), Some(p)) = (current_val, past_val) {
            let c_dec = Decimal::from_f64_retain(c);
            let p_dec = Decimal::from_f64_retain(p);

            if let (Some(c_d), Some(p_d)) = (c_dec, p_dec) {
                if p_d != Decimal::ZERO {
                    let roc_d = ((c_d - p_d) / p_d) * hundred;
                    *val = Some(roc_d.to_f64().unwrap_or(0.0));
                }
            }
        }
    }

    let s = Series::new("roc", roc_values);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 12.0, 15.0, 14.0, 18.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        // Period 2:
        // i=0: null
        // i=1: null
        // i=2: (15 - 10) / 10 * 100 = 50.0
        // i=3: (14 - 12) / 12 * 100 = 16.666...
        // i=4: (18 - 15) / 15 * 100 = 20.0

        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());
        assert_eq!(out.get(2), Some(50.0));

        let val3 = out.get(3).unwrap();
        assert!((val3 - 16.666666666666668).abs() < 1e-10);

        assert_eq!(out.get(4), Some(20.0));

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
        let df_short = df!("close" => &[10.0])?;
        let res_short = calculate(&df_short, 5)?;
        assert_eq!(res_short.len(), 1);
        assert!(res_short.f64()?.get(0).is_none());

        // Zero period
        let df_zero = df!("close" => &[10.0, 20.0])?;
        let res_zero = calculate(&df_zero, 0);
        assert!(res_zero.is_err());
        assert_eq!(
            res_zero.unwrap_err().to_string(),
            "Period must be greater than 0"
        );

        // Division by zero prevention
        let df_div_zero = df!("close" => &[0.0, 10.0])?;
        let res_div_zero = calculate(&df_div_zero, 1)?;
        let out_div_zero = res_div_zero.f64()?;
        assert!(out_div_zero.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "close" => &[100.0, 102.0, 101.0, 103.0, 102.0]
        )?;
        // Period 1
        // null
        // (102-100)/100*100 = 2.0
        // (101-102)/102*100 = -0.98039...
        // (103-101)/101*100 = 1.98019...
        // (102-103)/103*100 = -0.97087...
        let s = calculate(&df, 1)?;
        let out = s.f64()?;

        assert!(out.get(0).is_none());
        assert_eq!(out.get(1), Some(2.0));

        let val2 = out.get(2).unwrap();
        assert!((val2 - -0.9803921568627451).abs() < 1e-10);

        let val3 = out.get(3).unwrap();
        assert!((val3 - 1.9801980198019802).abs() < 1e-10);

        let val4 = out.get(4).unwrap();
        assert!((val4 - -0.970873786407767).abs() < 1e-10);

        Ok(())
    }
}
