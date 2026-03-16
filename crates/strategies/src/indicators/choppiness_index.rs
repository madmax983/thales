//! Choppiness Index (CHOP) - A volatility indicator designed to determine if the market is choppy (trading sideways) or not choppy (trading within a trend in either direction).

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Choppiness Index (CHOP)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns
/// * `period` - Lookback period (typically 14)
///
/// # Returns
/// Series with CHOP values.
///
/// # Example
/// ```rust
/// use anyhow::Result;
/// use polars::prelude::*;
/// use strategies::indicators::choppiness_index;
///
/// fn example() -> Result<()> {
///     let df = df!(
///         "high" => &[10.0, 11.0, 12.0, 12.0, 11.0, 10.0, 9.0],
///         "low" => &[8.0, 9.0, 10.0, 10.0, 9.0, 8.0, 7.0],
///         "close" => &[9.0, 10.0, 11.0, 11.0, 10.0, 9.0, 8.0]
///     )?;
///     let result = choppiness_index::calculate(&df, 3)?;
///     Ok(())
/// }
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period < 2 {
        anyhow::bail!("Period must be at least 2");
    }

    let high = data
        .column("high")
        .context("Missing 'high' column")?
        .f64()?;
    let low = data.column("low").context("Missing 'low' column")?.f64()?;
    let close = data
        .column("close")
        .context("Missing 'close' column")?
        .f64()?;

    // Ensure data validity
    if high.null_count() > 0 || low.null_count() > 0 || close.null_count() > 0 {
        anyhow::bail!("Data contains null values, cannot calculate CHOP");
    }

    let len = close.len();
    let mut chop_values: Vec<f64> = vec![f64::NAN; len];

    if len < period {
        return Ok(Series::new("choppiness_index", chop_values));
    }

    let mut tr_values: Vec<Decimal> = Vec::with_capacity(len);
    let mut high_decimals: Vec<Decimal> = Vec::with_capacity(len);
    let mut low_decimals: Vec<Decimal> = Vec::with_capacity(len);

    for i in 0..len {
        let h_f64 = high.get(i).context("Failed to get high value")?;
        let l_f64 = low.get(i).context("Failed to get low value")?;
        let c_f64 = close.get(i).context("Failed to get close value")?;

        if h_f64.is_nan() || l_f64.is_nan() || c_f64.is_nan() {
            anyhow::bail!("Data contains NaN values, cannot calculate CHOP");
        }

        let h = Decimal::from_f64_retain(h_f64).context("Failed to convert high to Decimal")?;
        let l = Decimal::from_f64_retain(l_f64).context("Failed to convert low to Decimal")?;

        high_decimals.push(h);
        low_decimals.push(l);

        if i == 0 {
            tr_values.push(h - l);
        } else {
            let prev_c_f64 = close.get(i - 1).context("Failed to get previous close")?;
            let prev_c = Decimal::from_f64_retain(prev_c_f64)
                .context("Failed to convert prev close to Decimal")?;
            let tr1 = h - l;
            let tr2 = (h - prev_c).abs();
            let tr3 = (l - prev_c).abs();
            let tr = tr1.max(tr2).max(tr3);
            tr_values.push(tr);
        }
    }

    let log10_period = Decimal::from(period).log10();
    let hundred = Decimal::from(100);

    // Optimize sliding window by keeping running sum
    let mut tr_sum = Decimal::ZERO;

    // Initialize the first window
    for &tr_val in tr_values.iter().take(period) {
        tr_sum += tr_val;
    }

    for i in (period - 1)..len {
        if i >= period {
            tr_sum += tr_values[i];
            tr_sum -= tr_values[i - period];
        }

        let mut max_h = Decimal::MIN;
        let mut min_l = Decimal::MAX;

        // O(period) rolling max/min - keeping it simple for small periods,
        // could use VecDeque for O(N) but period is usually 14 so this is fast enough.
        for j in (i + 1 - period)..=i {
            let h = high_decimals[j];
            let l = low_decimals[j];
            if h > max_h {
                max_h = h;
            }
            if l < min_l {
                min_l = l;
            }
        }

        let range = max_h - min_l;

        if !range.is_zero() && !tr_sum.is_zero() {
            let ratio = tr_sum / range;
            if ratio > Decimal::ZERO {
                let chop = hundred * ratio.log10() / log10_period;
                chop_values[i] = chop.to_f64().context("Failed to convert CHOP to f64")?;
            }
        }
    }

    Ok(Series::new("choppiness_index", chop_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 11.0, 12.0],
            "low" => &[8.0, 9.0, 10.0],
            "close" => &[9.0, 10.0, 11.0]
        )?;

        let result = calculate(&df, 3)?;
        let out = result.f64()?;
        assert_eq!(out.len(), 3);

        let val2 = out.get(2).context("Missing value")?;
        assert!((val2 - 36.907).abs() < 0.01);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty, 14).is_err());

        let df_single = df!(
            "high" => &[10.0],
            "low" => &[10.0],
            "close" => &[10.0]
        )?;
        let res = calculate(&df_single, 14)?;
        let out_single = res.f64()?;
        assert!(out_single.get(0).context("Missing value")?.is_nan());

        let df_flat = df!(
            "high" => &[10.0, 10.0, 10.0],
            "low" => &[10.0, 10.0, 10.0],
            "close" => &[10.0, 10.0, 10.0]
        )?;
        let res_flat = calculate(&df_flat, 3)?;
        let out_flat = res_flat.f64()?;
        assert!(out_flat.get(2).context("Missing value")?.is_nan());

        // Test NaN handling
        let df_nan = df!(
            "high" => &[10.0, f64::NAN],
            "low" => &[8.0, 9.0],
            "close" => &[9.0, 10.0]
        )?;
        assert!(calculate(&df_nan, 2).is_err());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let df = df!(
            "high" => &[10.5, 11.2, 10.8, 11.5, 12.0],
            "low" => &[10.0, 10.5, 10.2, 10.8, 11.0],
            "close" => &[10.2, 11.0, 10.5, 11.2, 11.8]
        )?;

        let result = calculate(&df, 3)?;
        assert_eq!(result.len(), 5);
        let out = result.f64()?;

        assert!(out.get(0).context("Missing value")?.is_nan());
        assert!(out.get(1).context("Missing value")?.is_nan());
        assert!(!out.get(2).context("Missing value")?.is_nan());
        assert!(!out.get(3).context("Missing value")?.is_nan());
        assert!(!out.get(4).context("Missing value")?.is_nan());

        Ok(())
    }
}
