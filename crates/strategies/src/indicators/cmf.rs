use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Chaikin Money Flow (CMF)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close", "volume" columns
/// * `period` - Lookback period (standard is 21)
///
/// # Returns
/// Series with CMF values. The first `period - 1` values will be null.
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
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

    let volume = data
        .column("volume")
        .context("DataFrame must contain 'volume' column")?
        .f64()
        .context("Volume column must be numeric (f64)")?;

    let len = close.len();
    let mut cmf_values: Vec<Option<f64>> = vec![None; len];

    if len < period {
        return Ok(Series::new("cmf", cmf_values));
    }

    // Prepare Decimals for calculation
    let mut money_flow_volumes: Vec<Decimal> = Vec::with_capacity(len);
    let mut volumes: Vec<Decimal> = Vec::with_capacity(len);

    for i in 0..len {
        let h = Decimal::from_f64_retain(high.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let l = Decimal::from_f64_retain(low.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let c = Decimal::from_f64_retain(close.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let v =
            Decimal::from_f64_retain(volume.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);

        let high_low_diff = h - l;

        let mfm = if high_low_diff.is_zero() {
            Decimal::ZERO
        } else {
            // Money Flow Multiplier = ((Close - Low) - (High - Close)) / (High - Low)
            ((c - l) - (h - c)) / high_low_diff
        };

        // Money Flow Volume = MFM * Volume
        let mfv = mfm * v;
        money_flow_volumes.push(mfv);
        volumes.push(v);
    }

    // Efficient rolling sum
    // Initialize first window sum
    let mut sum_mfv = Decimal::ZERO;
    let mut sum_vol = Decimal::ZERO;

    // Sum first 'period' elements (indices 0 to period - 1)
    for i in 0..period {
        sum_mfv += money_flow_volumes[i];
        sum_vol += volumes[i];
    }

    // Calculate CMF at index `period - 1`
    let cmf = if sum_vol.is_zero() {
        Decimal::ZERO
    } else {
        sum_mfv.checked_div(sum_vol).unwrap_or(Decimal::ZERO)
    };
    cmf_values[period - 1] = cmf.to_f64();

    // Slide window
    for i in period..len {
        // Add new
        sum_mfv += money_flow_volumes[i];
        sum_vol += volumes[i];

        // Remove old (element at i - period)
        sum_mfv -= money_flow_volumes[i - period];
        sum_vol -= volumes[i - period];

        let cmf = if sum_vol.is_zero() {
            Decimal::ZERO
        } else {
            sum_mfv.checked_div(sum_vol).unwrap_or(Decimal::ZERO)
        };
        cmf_values[i] = cmf.to_f64();
    }

    Ok(Series::new("cmf", cmf_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_cmf_calculation() -> Result<()> {
        // i=0: H=12, L=10, C=11. diff=2. MFM = ((11-10) - (12-11))/2 = (1 - 1)/2 = 0. MFV = 0. V=100.
        // i=1: H=12, L=10, C=12. diff=2. MFM = ((12-10) - (12-12))/2 = (2 - 0)/2 = 1. MFV = 100. V=100.
        // i=2: H=12, L=10, C=10. diff=2. MFM = ((10-10) - (12-10))/2 = (0 - 2)/2 = -1. MFV = -100. V=100.

        // Period = 2
        // i=0: None
        // i=1: Window[0,1]. SumMFV = 0 + 100 = 100. SumV = 100 + 100 = 200. CMF = 100/200 = 0.5.
        // i=2: Window[1,2]. SumMFV = 100 - 100 = 0. SumV = 100 + 100 = 200. CMF = 0/200 = 0.0.

        let df = df!(
            "high" => &[12.0, 12.0, 12.0],
            "low" => &[10.0, 10.0, 10.0],
            "close" => &[11.0, 12.0, 10.0],
            "volume" => &[100.0, 100.0, 100.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 3);
        assert!(out.get(0).is_none());

        // i=1
        let val1 = out.get(1).unwrap();
        assert!((val1 - 0.5).abs() < 1e-6);

        // i=2
        let val2 = out.get(2).unwrap();
        assert!((val2 - 0.0).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn test_edge_cases() {
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty, 14).is_err());

        // Single row
        let df_short = df!(
            "high" => &[10.0],
            "low" => &[10.0],
            "close" => &[10.0],
            "volume" => &[100.0]
        )
        .unwrap();
        let res = calculate(&df_short, 5).unwrap();
        assert!(res.f64().unwrap().get(0).is_none());

        // High == Low
        let df_flat = df!(
            "high" => &[10.0, 10.0],
            "low" => &[10.0, 10.0],
            "close" => &[10.0, 10.0],
            "volume" => &[100.0, 100.0]
        )
        .unwrap();
        let res_flat = calculate(&df_flat, 2).unwrap();
        let out_flat = res_flat.f64().unwrap();
        assert_eq!(out_flat.get(1).unwrap(), 0.0);
    }
}
