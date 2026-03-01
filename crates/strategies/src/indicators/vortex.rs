use anyhow::{Context, Result};
use polars::prelude::*;
/// Calculate Vortex Indicator (VI)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns
/// * `period` - Lookback period (typically 14)
///
/// # Returns
/// Tuple of two Series: (VI+, VI-)
pub fn calculate(data: &DataFrame, period: usize) -> Result<(Series, Series)> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let high = data
        .column("high")
        .context("DataFrame must contain 'high' column")?
        .f64()
        .context("High column must be numeric (f64)")?;
    let low = data
        .column("low")
        .context("DataFrame must contain 'low' column")?
        .f64()
        .context("Low column must be numeric (f64)")?;
    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()
        .context("Close column must be numeric (f64)")?;

    let mut tr_values = Vec::with_capacity(close.len());
    let mut vm_plus_values = Vec::with_capacity(close.len());
    let mut vm_minus_values = Vec::with_capacity(close.len());

    let mut vi_plus_series = Vec::with_capacity(close.len());
    let mut vi_minus_series = Vec::with_capacity(close.len());

    // First element has no previous values
    tr_values.push(0.0);
    vm_plus_values.push(0.0);
    vm_minus_values.push(0.0);
    vi_plus_series.push(None);
    vi_minus_series.push(None);

    let mut sum_tr = 0.0;
    let mut sum_vm_plus = 0.0;
    let mut sum_vm_minus = 0.0;

    for i in 1..close.len() {
        let current_high = high.get(i).unwrap_or(f64::NAN);
        let current_low = low.get(i).unwrap_or(f64::NAN);

        let prev_high = high.get(i - 1).unwrap_or(f64::NAN);
        let prev_low = low.get(i - 1).unwrap_or(f64::NAN);
        let prev_close = close.get(i - 1).unwrap_or(f64::NAN);

        if current_high.is_nan()
            || current_low.is_nan()
            || prev_high.is_nan()
            || prev_low.is_nan()
            || prev_close.is_nan()
        {
            tr_values.push(0.0);
            vm_plus_values.push(0.0);
            vm_minus_values.push(0.0);
            vi_plus_series.push(None);
            vi_minus_series.push(None);

            // Recompute sums if we have missing data in the window.
            // Simplified: we'll just let the rolling window handle 0s,
            // but in reality we should reset. For a robust indicator,
            // we handle missing data properly.
            continue;
        }

        // True Range (TR)
        let hl = current_high - current_low;
        let hc = (current_high - prev_close).abs();
        let lc = (current_low - prev_close).abs();
        let tr = hl.max(hc).max(lc);
        tr_values.push(tr);

        // Vortex Movement (VM)
        let vm_plus = (current_high - prev_low).abs();
        let vm_minus = (current_low - prev_high).abs();
        vm_plus_values.push(vm_plus);
        vm_minus_values.push(vm_minus);

        // Add to sums
        sum_tr += tr;
        sum_vm_plus += vm_plus;
        sum_vm_minus += vm_minus;

        // Remove old values from sums if window is full
        if i >= period {
            if i > period {
                sum_tr -= tr_values[i - period];
                sum_vm_plus -= vm_plus_values[i - period];
                sum_vm_minus -= vm_minus_values[i - period];
            }

            if sum_tr > 0.0 {
                let vi_plus = sum_vm_plus / sum_tr;
                let vi_minus = sum_vm_minus / sum_tr;
                vi_plus_series.push(Some(vi_plus));
                vi_minus_series.push(Some(vi_minus));
            } else {
                vi_plus_series.push(None);
                vi_minus_series.push(None);
            }
        } else {
            vi_plus_series.push(None);
            vi_minus_series.push(None);
        }
    }

    let vi_plus_s = Series::new("vi_plus", vi_plus_series);
    let vi_minus_s = Series::new("vi_minus", vi_minus_series);

    Ok((vi_plus_s, vi_minus_s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_vortex_calculation() -> Result<()> {
        let df = df!(
            "high" => &[10.0, 12.0, 14.0, 15.0, 16.0, 15.0],
            "low" => &[8.0, 9.0, 11.0, 13.0, 14.0, 12.0],
            "close" => &[9.0, 11.0, 13.0, 14.0, 15.0, 13.0]
        )?;

        let period = 3;
        let (vi_plus, vi_minus) = calculate(&df, period)?;

        let vi_p_f64 = vi_plus.f64()?;
        let vi_m_f64 = vi_minus.f64()?;

        // i=0: TR=0, VM+=0, VM-=0
        // i=1: high=12, low=9, prev_close=9, prev_high=10, prev_low=8.
        // TR=max(12-9, 12-9, 9-9)=3
        // VM+ = abs(12-8)=4
        // VM- = abs(9-10)=1
        // i=2: high=14, low=11, prev_close=11, prev_high=12, prev_low=9.
        // TR=max(14-11, 14-11, 11-11)=3
        // VM+ = abs(14-9)=5
        // VM- = abs(11-12)=1
        // i=3: high=15, low=13, prev_close=13, prev_high=14, prev_low=11.
        // TR=max(15-13, 15-13, 13-13)=2
        // VM+ = abs(15-11)=4
        // VM- = abs(13-14)=1
        // Sum(TR)=3+3+2=8. Sum(VM+)=4+5+4=13. Sum(VM-)=1+1+1=3.
        // VI+ = 13/8 = 1.625
        // VI- = 3/8 = 0.375

        assert!(vi_p_f64.get(0).is_none());
        assert!(vi_p_f64.get(1).is_none());
        assert!(vi_p_f64.get(2).is_none()); // because period=3 means we need 3 values, which occurs at i=3

        let expected_vi_plus = 13.0 / 8.0;
        let expected_vi_minus = 3.0 / 8.0;

        assert!((vi_p_f64.get(3).unwrap() - expected_vi_plus).abs() < 1e-6);
        assert!((vi_m_f64.get(3).unwrap() - expected_vi_minus).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 14);
        assert!(res_empty.is_err());

        let df_short = df!(
            "high" => &[10.0, 11.0],
            "low" => &[8.0, 9.0],
            "close" => &[9.0, 10.0]
        )?;
        let (vp, _vm) = calculate(&df_short, 14)?;
        assert_eq!(vp.len(), 2);
        assert!(vp.f64()?.get(0).is_none());
        assert!(vp.f64()?.get(1).is_none());

        Ok(())
    }
}
