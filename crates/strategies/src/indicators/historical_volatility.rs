//! Historical Volatility (HV)
//!
//! Calculates the annualized Historical Volatility based on the standard deviation
//! of logarithmic returns over a specified lookback period.
//!
//! # Rationale
//! Historical Volatility is a statistical measure of the dispersion of returns for a given security
//! or market index. In most cases, the higher the volatility, the riskier the security.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Historical Volatility (HV)
///
/// # Arguments
/// * `data` - DataFrame with "close" column
/// * `period` - Lookback period (e.g., 20)
/// * `annualization_factor` - Typically 365 for crypto or 252 for traditional equities
///
/// # Returns
/// Series with Historical Volatility values. The first `period` values will be null.
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// // let df = ...;
/// // let hv = strategies::indicators::historical_volatility::calculate(&df, 20, 365.0)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize, annualization_factor: f64) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period < 2 {
        anyhow::bail!("Period must be at least 2");
    }

    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let close_vec: Vec<Option<f64>> = close.into_iter().collect();
    let mut hv_values: Vec<Option<f64>> = Vec::with_capacity(close.len());

    // Calculate log returns
    let mut log_returns: Vec<Option<Decimal>> = Vec::with_capacity(close.len());
    for i in 0..close.len() {
        if i == 0 {
            log_returns.push(None);
            continue;
        }

        match (close_vec[i], close_vec[i - 1]) {
            (Some(curr), Some(prev)) if curr > 0.0 && prev > 0.0 => {
                let r = (curr / prev).ln();
                log_returns.push(Decimal::from_f64_retain(r));
            }
            _ => log_returns.push(None),
        }
    }

    let ann_factor_dec = Decimal::from_f64_retain(annualization_factor.sqrt())
        .context("Invalid annualization factor")?;

    let period_dec = Decimal::from_usize(period).context("Invalid period")?;
    let period_minus_one_dec = Decimal::from_usize(period - 1).context("Invalid period - 1")?;

    for i in 0..close.len() {
        if i < period {
            hv_values.push(None);
            continue;
        }

        let mut sum = Decimal::ZERO;
        let mut sum_sq = Decimal::ZERO;
        let mut valid_count = 0;

        // Window for log returns is from i - period + 1 to i inclusive
        for j in (i - period + 1)..=i {
            if let Some(r) = log_returns[j] {
                sum += r;
                sum_sq += r * r;
                valid_count += 1;
            }
        }

        if valid_count == period {
            let mean = sum / period_dec;
            let mut variance = Decimal::ZERO;
            for j in (i - period + 1)..=i {
                if let Some(r) = log_returns[j] {
                    let diff = r - mean;
                    variance += diff * diff;
                }
            }
            variance /= period_minus_one_dec;

            let std_dev = variance.to_f64().unwrap_or(0.0).sqrt();
            let hv = Decimal::from_f64_retain(std_dev).unwrap_or(Decimal::ZERO) * ann_factor_dec;
            hv_values.push(hv.to_f64());
        } else {
            hv_values.push(None);
        }
    }

    Ok(Series::new("historical_volatility", hv_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_historical_volatility() -> Result<()> {
        let df = df!(
            "close" => &[10.0, 10.5, 10.2, 10.8, 11.0, 10.9]
        )?;

        let result = calculate(&df, 3, 252.0)?;
        let hv = result.f64()?;

        assert!(hv.get(0).is_none());
        assert!(hv.get(1).is_none());
        assert!(hv.get(2).is_none());

        let val3 = hv.get(3).unwrap();
        assert!((val3 - 0.754).abs() < 0.01);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14, 365.0);
        assert!(res_empty.is_err());

        let df_short = df!("close" => &[10.0, 11.0])?;
        let res_short = calculate(&df_short, 5, 365.0)?;
        assert_eq!(res_short.len(), 2);
        assert!(res_short.f64()?.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let values: Vec<f64> = (0..50).map(|i| 100.0 + (i as f64 * 0.1).sin() * 5.0).collect();
        let df = df!("close" => values)?;
        let result = calculate(&df, 14, 365.0)?;
        let s = result.f64()?;

        assert_eq!(s.len(), 50);
        assert!(s.get(13).is_none());
        assert!(s.get(14).is_some());

        Ok(())
    }
}
