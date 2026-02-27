use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Money Flow Index (MFI)
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close", "volume" columns
/// * `period` - Lookback period (standard is 14)
///
/// # Returns
/// Series with MFI values. The first `period` values will be null.
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
    let mut mfi_values: Vec<Option<f64>> = vec![None; len];

    if len <= period {
        return Ok(Series::new("mfi", mfi_values));
    }

    // Prepare Decimals for calculation
    let mut typical_prices: Vec<Decimal> = Vec::with_capacity(len);
    let mut raw_money_flows: Vec<Decimal> = Vec::with_capacity(len);

    for i in 0..len {
        let h = Decimal::from_f64_retain(high.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let l = Decimal::from_f64_retain(low.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let c = Decimal::from_f64_retain(close.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let v = Decimal::from_f64_retain(volume.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);

        // Typical Price = (High + Low + Close) / 3
        let tp = (h + l + c) / Decimal::from(3);
        typical_prices.push(tp);

        // Raw Money Flow = Typical Price * Volume
        let rmf = tp * v;
        raw_money_flows.push(rmf);
    }

    // Calculate Positive and Negative Money Flows
    let mut positive_flows: Vec<Decimal> = vec![Decimal::ZERO; len];
    let mut negative_flows: Vec<Decimal> = vec![Decimal::ZERO; len];

    // Start from 1 because we compare with previous
    for i in 1..len {
        if typical_prices[i] > typical_prices[i - 1] {
            positive_flows[i] = raw_money_flows[i];
        } else if typical_prices[i] < typical_prices[i - 1] {
            negative_flows[i] = raw_money_flows[i];
        }
        // If equal, both are 0 (disregarded)
    }

    // Calculate sums for the initial period
    // We need sum over period.
    // The first value of MFI is at index `period`.
    // It sums flows from index 1 to period (inclusive) - Wait, standard definition is last 14 periods.
    // If period is 14.
    // Index 14 uses flows from 1 to 14?
    // Let's check window logic.
    // Window of size 14 ending at i means i-13 to i.

    // Efficient rolling sum
    // Initialize first window sum
    let mut sum_pos = Decimal::ZERO;
    let mut sum_neg = Decimal::ZERO;

    // Sum first 'period' elements (indices 1 to period)
    // Note: index 0 has no flow because no previous price.
    // So for the first window (ending at index `period`), we sum indices 1..=period.

    for i in 1..=period {
        if i < len {
            sum_pos += positive_flows[i];
            sum_neg += negative_flows[i];
        }
    }

    let hundred = Decimal::from(100);

    // Calculate MFI at index `period`
    if len > period {
        let mfi = if sum_neg.is_zero() {
            hundred
        } else {
            let mfr = sum_pos.checked_div(sum_neg).unwrap_or(Decimal::ZERO);
            hundred - (hundred / (Decimal::ONE + mfr))
        };
        mfi_values[period] = mfi.to_f64();
    }

    // Slide window
    for i in (period + 1)..len {
        // Add new
        sum_pos += positive_flows[i];
        sum_neg += negative_flows[i];

        // Remove old (element at i - period)
        sum_pos -= positive_flows[i - period];
        sum_neg -= negative_flows[i - period];

        // Ensure non-negative due to potential floating point issues (though using Decimal helps)
        if sum_pos < Decimal::ZERO { sum_pos = Decimal::ZERO; }
        if sum_neg < Decimal::ZERO { sum_neg = Decimal::ZERO; }

        let mfi = if sum_neg.is_zero() {
            hundred
        } else {
            let mfr = sum_pos.checked_div(sum_neg).unwrap_or(Decimal::ZERO);
            let denominator = Decimal::ONE + mfr;
            if denominator.is_zero() {
                hundred
            } else {
                hundred - (hundred.checked_div(denominator).unwrap_or(Decimal::ZERO))
            }
        };
        mfi_values[i] = mfi.to_f64();
    }

    Ok(Series::new("mfi", mfi_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_mfi_calculation() -> Result<()> {
        // High, Low, Close, Volume
        // TP = (H+L+C)/3
        // RMF = TP * V
        // Flow direction based on TP change

        // i=0: TP=10. No flow.
        // i=1: TP=11 (>10). Pos=11*100=1100. Neg=0.
        // i=2: TP=12 (>11). Pos=12*100=1200. Neg=0.
        // Period=2.
        // At i=2: Window [1, 2].
        // SumPos = 1100+1200 = 2300.
        // SumNeg = 0.
        // MFI = 100.

        // i=3: TP=11 (<12). Pos=0. Neg=11*100=1100.
        // At i=3: Window [2, 3].
        // Remove i=1 (Pos 1100, Neg 0).
        // Add i=3 (Pos 0, Neg 1100).
        // SumPos = 2300 - 1100 + 0 = 1200.
        // SumNeg = 0 - 0 + 1100 = 1100.
        // MFR = 1200/1100 = 1.0909...
        // MFI = 100 - (100 / 2.0909) = 100 - 47.82 = 52.17

        let df = df!(
            "high" => &[10.0, 11.0, 12.0, 11.0],
            "low" => &[10.0, 11.0, 12.0, 11.0],
            "close" => &[10.0, 11.0, 12.0, 11.0],
            "volume" => &[100.0, 100.0, 100.0, 100.0]
        )?;

        let result = calculate(&df, 2)?;
        let out = result.f64()?;

        assert_eq!(out.len(), 4);
        assert!(out.get(0).is_none());
        assert!(out.get(1).is_none());

        // i=2
        assert_eq!(out.get(2), Some(100.0));

        // i=3
        let val3 = out.get(3).unwrap();
        // 100 - (100 / (1 + 1200/1100)) = 100 - (100 / 2.090909) = 100 - 47.826 = 52.1739
        assert!(
            (val3 - 52.1739).abs() < 1e-3,
            "Expected ~52.1739, got {}", val3
        );

        Ok(())
    }

    #[test]
    fn test_edge_cases() {
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty, 14).is_err());

        let df_short = df!(
            "high" => &[10.0],
            "low" => &[10.0],
            "close" => &[10.0],
            "volume" => &[100.0]
        ).unwrap();
        let res = calculate(&df_short, 5).unwrap();
        assert!(res.f64().unwrap().get(0).is_none());
    }
}
