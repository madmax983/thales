//! Average Directional Index (ADX)
//!
//! Calculates the ADX, +DI, and -DI indicators.
//! Used to determine trend strength and direction.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate ADX, +DI, and -DI
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns
/// * `period` - Lookback period (standard is 14)
///
/// # Returns
/// Tuple of (ADX, +DI, -DI) Series.
pub fn calculate(data: &DataFrame, period: usize) -> Result<(Series, Series, Series)> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }

    let high = data.column("high").context("Missing 'high'")?.f64()?;
    let low = data.column("low").context("Missing 'low'")?.f64()?;
    let close = data.column("close").context("Missing 'close'")?.f64()?;

    let len = close.len();

    // Output vectors
    let mut adx_values: Vec<Option<f64>> = vec![None; len];
    let mut plus_di_values: Vec<Option<f64>> = vec![None; len];
    let mut minus_di_values: Vec<Option<f64>> = vec![None; len];

    if len < period * 2 {
        // Need enough data for ADX smoothing (approx 2*period)
        return Ok((
            Series::new("adx", adx_values),
            Series::new("plus_di", plus_di_values),
            Series::new("minus_di", minus_di_values),
        ));
    }

    let period_dec = Decimal::from_usize(period).unwrap();
    let period_minus_one = period_dec - Decimal::ONE;

    // Vectors for intermediate calculations
    let mut tr_vec = Vec::with_capacity(len);
    let mut plus_dm_vec = Vec::with_capacity(len);
    let mut minus_dm_vec = Vec::with_capacity(len);

    // 1. Calculate TR, +DM, -DM for all bars
    // First bar has no previous close, so TR=High-Low, DM=0
    // Actually, Wilder starts calculation from 2nd bar (index 1).
    // Let's treat index 0 as valid but TR is just H-L, DM is 0.

    // Index 0
    let h0 = Decimal::from_f64_retain(high.get(0).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
    let l0 = Decimal::from_f64_retain(low.get(0).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);

    tr_vec.push(h0 - l0);
    plus_dm_vec.push(Decimal::ZERO);
    minus_dm_vec.push(Decimal::ZERO);

    for i in 1..len {
        let h_curr =
            Decimal::from_f64_retain(high.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let l_curr =
            Decimal::from_f64_retain(low.get(i).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let h_prev =
            Decimal::from_f64_retain(high.get(i - 1).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let l_prev =
            Decimal::from_f64_retain(low.get(i - 1).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);
        let c_prev =
            Decimal::from_f64_retain(close.get(i - 1).unwrap_or(f64::NAN)).unwrap_or(Decimal::ZERO);

        // TR
        let hl = h_curr - l_curr;
        let hcp = (h_curr - c_prev).abs();
        let lcp = (l_curr - c_prev).abs();
        let tr = hl.max(hcp).max(lcp);
        tr_vec.push(tr);

        // DM
        let up_move = h_curr - h_prev;
        let down_move = l_prev - l_curr;

        let mut plus_dm = Decimal::ZERO;
        let mut minus_dm = Decimal::ZERO;

        if up_move > down_move && up_move > Decimal::ZERO {
            plus_dm = up_move;
        }
        if down_move > up_move && down_move > Decimal::ZERO {
            minus_dm = down_move;
        }

        plus_dm_vec.push(plus_dm);
        minus_dm_vec.push(minus_dm);
    }

    // 2. Smooth TR, +DM, -DM
    // Initial value is sum of first `period` values (starting from index 0? usually from index 1?)
    // Standard implementations usually skip index 0 for DM calculations if they depend on prev.
    // We calculated DM[0] as 0. So sum from 0 to period-1?
    // Wilder says "The first TR14 is the sum of the first 14 TR1s".
    // Let's sum from index 1 to period. (Since index 0 has no true range relative to prev close, but it has range).
    // Actually, TR at index 0 is valid range. DM at index 0 is 0.
    // Let's sum from index 0 to period-1.

    let mut smoothed_tr = Decimal::ZERO;
    let mut smoothed_plus_dm = Decimal::ZERO;
    let mut smoothed_minus_dm = Decimal::ZERO;

    for i in 0..period {
        smoothed_tr += tr_vec[i];
        smoothed_plus_dm += plus_dm_vec[i];
        smoothed_minus_dm += minus_dm_vec[i];
    }

    // Wilder's Smoothing Update uses (Prev * (N-1) + Curr) / N
    // This expects Prev to be an AVERAGE, not a SUM.
    // So we must initialize with the SMA (Sum / N).
    smoothed_tr /= period_dec;
    smoothed_plus_dm /= period_dec;
    smoothed_minus_dm /= period_dec;

    // Store DX values to smooth later
    // We can't calculate ADX until we have smoothed DX.
    // DX is calculated from smoothed DMs/TRs.

    // We need to store smoothed values to evolve them.
    let mut dx_vec: Vec<Option<Decimal>> = vec![None; len];

    // First DX value is at index `period - 1`
    // DX = 100 * |+DI - -DI| / (+DI + -DI)
    // DI = 100 * SmoothedDM / SmoothedTR
    let calc_dx = |p_dm: Decimal, m_dm: Decimal, tr: Decimal| -> Option<Decimal> {
        if tr == Decimal::ZERO {
            return None;
        }
        let p_di = (p_dm / tr) * Decimal::from(100);
        let m_di = (m_dm / tr) * Decimal::from(100);
        let sum = p_di + m_di;
        if sum == Decimal::ZERO {
            return Some(Decimal::ZERO);
        }
        Some(((p_di - m_di).abs() / sum) * Decimal::from(100))
    };

    if let Some(dx) = calc_dx(smoothed_plus_dm, smoothed_minus_dm, smoothed_tr) {
        dx_vec[period - 1] = Some(dx);

        // Output DIs for this index
        if smoothed_tr != Decimal::ZERO {
            plus_di_values[period - 1] = Some(
                ((smoothed_plus_dm / smoothed_tr) * Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0),
            );
            minus_di_values[period - 1] = Some(
                ((smoothed_minus_dm / smoothed_tr) * Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0),
            );
        }
    }

    // Iterate for rest
    for i in period..len {
        let tr = tr_vec[i];
        let p_dm = plus_dm_vec[i];
        let m_dm = minus_dm_vec[i];

        // Wilder's Smoothing: Next = Prev - (Prev/n) + Curr
        // Which is: (Prev * (n-1) + Curr) / n
        smoothed_tr = (smoothed_tr * period_minus_one + tr) / period_dec;
        smoothed_plus_dm = (smoothed_plus_dm * period_minus_one + p_dm) / period_dec;
        smoothed_minus_dm = (smoothed_minus_dm * period_minus_one + m_dm) / period_dec;

        if let Some(dx) = calc_dx(smoothed_plus_dm, smoothed_minus_dm, smoothed_tr) {
            dx_vec[i] = Some(dx);

            // Output DIs
            if smoothed_tr != Decimal::ZERO {
                plus_di_values[i] = Some(
                    ((smoothed_plus_dm / smoothed_tr) * Decimal::from(100))
                        .to_f64()
                        .unwrap_or(0.0),
                );
                minus_di_values[i] = Some(
                    ((smoothed_minus_dm / smoothed_tr) * Decimal::from(100))
                        .to_f64()
                        .unwrap_or(0.0),
                );
            }
        }
    }

    // 3. Calculate ADX (Smoothed DX)
    // First ADX is average of first `period` DX values.
    // But we only started getting DX values at index `period - 1`.
    // So we need `period` DX values starting from there.
    // The first ADX will be available at index (period - 1) + (period - 1) = 2*period - 2.
    // Actually, usually ADX is available at index 2*period - 1.

    // Let's sum DXs.
    // Start index for DXs is `period - 1`.
    let dx_start_idx = period - 1;
    let adx_start_idx = dx_start_idx + period - 1;

    if len <= adx_start_idx {
        return Ok((
            Series::new("adx", adx_values),
            Series::new("plus_di", plus_di_values),
            Series::new("minus_di", minus_di_values),
        ));
    }

    let mut dx_sum = Decimal::ZERO;
    let mut valid_dx_count = 0;

    for val in dx_vec.iter().skip(dx_start_idx).take(period).flatten() {
        dx_sum += val;
        valid_dx_count += 1;
    }

    if valid_dx_count == period {
        let mut smoothed_adx = dx_sum / period_dec;
        adx_values[adx_start_idx] = Some(smoothed_adx.to_f64().unwrap_or(0.0));

        // Subsequent ADXs
        for i in (adx_start_idx + 1)..len {
            if let Some(dx) = dx_vec[i] {
                // Wilder's Smoothing for ADX
                // ADX[i] = (ADX[i-1] * (n-1) + DX[i]) / n
                smoothed_adx = (smoothed_adx * period_minus_one + dx) / period_dec;
                adx_values[i] = Some(smoothed_adx.to_f64().unwrap_or(0.0));
            }
        }
    }

    Ok((
        Series::new("adx", adx_values),
        Series::new("plus_di", plus_di_values),
        Series::new("minus_di", minus_di_values),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adx_calculation() -> Result<()> {
        // Values from a known ADX calculation (e.g. spreadsheet or other lib)
        // Let's create a trend.
        let values: Vec<f64> = (0..50).map(|i| 100.0 + i as f64).collect(); // Strong uptrend
        let highs: Vec<f64> = values.iter().map(|v| v + 1.0).collect();
        let lows: Vec<f64> = values.iter().map(|v| v - 1.0).collect();
        let closes = values;

        let df = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let (adx, p_di, m_di) = calculate(&df, 14)?;

        let adx_vec: Vec<Option<f64>> = adx.f64()?.into_iter().collect();
        let p_di_vec: Vec<Option<f64>> = p_di.f64()?.into_iter().collect();
        let m_di_vec: Vec<Option<f64>> = m_di.f64()?.into_iter().collect();

        // 14 period.
        // ADX should be available around index 27 (13 + 13 + 1?)
        // First DX at index 13.
        // First ADX at index 13 + 13 = 26.

        assert!(adx_vec[25].is_none());
        assert!(adx_vec[26].is_some());

        // In a strong uptrend:
        // +DI should be high
        // -DI should be low (near 0)
        // ADX should be increasing and high.

        let final_p_di = p_di_vec.last().unwrap().unwrap();
        let final_m_di = m_di_vec.last().unwrap().unwrap();
        let final_adx = adx_vec.last().unwrap().unwrap();

        assert!(final_p_di > final_m_di);
        assert!(final_adx > 25.0); // Strong trend
        assert!(final_p_di >= 49.0); // Very strong +DM (Theoretical max for this data is 50.0)

        Ok(())
    }

    #[test]
    fn test_empty_input() {
        let df = DataFrame::default();
        let res = calculate(&df, 14);
        assert!(res.is_err());
    }
}
