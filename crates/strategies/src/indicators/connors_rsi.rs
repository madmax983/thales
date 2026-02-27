//! Connors RSI
//!
//! Calculates the Connors RSI (CRSI), a composite indicator consisting of:
//! 1. RSI (3-period standard)
//! 2. RSI of Streak (2-period)
//! 3. Percent Rank of Rate of Change (100-period)
//!
//! Formula: (RSI + RSI(Streak) + PercentRank) / 3

use crate::indicators::rsi;
use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Connors RSI
///
/// # Arguments
/// * `data` - DataFrame with "close" column.
/// * `rsi_period` - Period for standard RSI (typically 3).
/// * `streak_rsi_period` - Period for RSI of the Streak (typically 2).
/// * `rank_lookback` - Lookback period for Percent Rank (typically 100).
///
/// # Returns
/// Series with Connors RSI values.
pub fn calculate(
    data: &DataFrame,
    rsi_period: usize,
    streak_rsi_period: usize,
    rank_lookback: usize,
) -> Result<Series> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if rsi_period == 0 || streak_rsi_period == 0 || rank_lookback == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

    // 1. Calculate Standard RSI
    let rsi_series = rsi::calculate(data, rsi_period)
        .context("Failed to calculate standard RSI")?;
    let rsi_arr = rsi_series.f64()?;

    // 2. Calculate Streak and RSI(Streak)
    let close = data
        .column("close")
        .context("DataFrame must contain 'close' column")?
        .f64()?;

    let mut streaks = Vec::with_capacity(close.len());
    let mut current_streak = 0.0;

    // First element has streak 0 (no prev comparison)
    streaks.push(0.0);

    for i in 1..close.len() {
        let curr_opt = close.get(i);
        let prev_opt = close.get(i - 1);

        if let (Some(curr), Some(prev)) = (curr_opt, prev_opt) {
            if curr > prev {
                if current_streak >= 0.0 {
                    current_streak += 1.0;
                } else {
                    current_streak = 1.0;
                }
            } else if curr < prev {
                if current_streak <= 0.0 {
                    current_streak -= 1.0;
                } else {
                    current_streak = -1.0;
                }
            } else {
                current_streak = 0.0;
            }
        } else {
            current_streak = 0.0;
        }
        streaks.push(current_streak);
    }

    // Create temp DF for RSI(Streak) calculation
    // RSI expects "close" column
    let streak_df = df!("close" => streaks.clone())?;
    let streak_rsi_series = rsi::calculate(&streak_df, streak_rsi_period)
        .context("Failed to calculate RSI(Streak)")?;
    let streak_rsi_arr = streak_rsi_series.f64()?;

    // 3. Calculate Percent Rank
    // Percent Rank of one-day return (ROC1)
    // ROC = (Close[i] - Close[i-1]) / Close[i-1] (or just returns)
    // Wait, Connors RSI typically uses Returns (or just Price Change, result is similar for Rank).
    // Let's use Returns.

    let mut returns = Vec::with_capacity(close.len());
    returns.push(None); // No return for first element

    for i in 1..close.len() {
        let curr_opt = close.get(i);
        let prev_opt = close.get(i - 1);

        if let (Some(curr), Some(prev)) = (curr_opt, prev_opt) {
            if prev != 0.0 {
                returns.push(Some((curr - prev) / prev));
            } else {
                returns.push(None);
            }
        } else {
            returns.push(None);
        }
    }

    let mut percent_ranks = Vec::with_capacity(close.len());

    // Rolling Rank
    // Ideally O(N) sliding window, but O(N * Lookback) is acceptable here.
    // Lookback = 100.

    for i in 0..close.len() {
        if i < rank_lookback {
            percent_ranks.push(None);
            continue;
        }

        // Current return
        let current_ret_opt = returns[i];

        if let Some(current_ret) = current_ret_opt {
            let start_idx = i - rank_lookback;
            let end_idx = i; // Excluding current? Definition: "percentage of values in the lookback period"
            // Usually lookback is previous N days.
            // ConnorsRSI definition: "Percent Rank of the one-day return over the past 100 days".
            // Does it include today? Usually PercentRank(x, N) compares x against previous N values.

            let mut count_lt = 0;
            let mut count_total = 0;

            for j in start_idx..end_idx {
                if let Some(ret) = returns[j] {
                    count_total += 1;
                    if ret < current_ret {
                        count_lt += 1;
                    }
                }
            }

            if count_total > 0 {
                percent_ranks.push(Some((count_lt as f64 / count_total as f64) * 100.0));
            } else {
                percent_ranks.push(None);
            }
        } else {
            percent_ranks.push(None);
        }
    }

    // 4. Combine: (RSI + RSI(Streak) + PercentRank) / 3
    let mut crsi_values = Vec::with_capacity(close.len());

    for i in 0..close.len() {
        let rsi_val = rsi_arr.get(i);
        let streak_rsi_val = streak_rsi_arr.get(i);
        let rank_val = percent_ranks[i];

        if let (Some(r), Some(sr), Some(pr)) = (rsi_val, streak_rsi_val, rank_val) {
            let crsi = (r + sr + pr) / 3.0;
            crsi_values.push(Some(crsi));
        } else {
            crsi_values.push(None);
        }
    }

    Ok(Series::new("connors_rsi", crsi_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_connors_rsi_calculation() -> Result<()> {
        // Generate synthetic data
        // 1. Uptrend for Streak
        // 2. Volatile for RSI
        // 3. Returns for Rank

        let mut closes = Vec::new();
        let mut val = 100.0;

        // 0-10: Flat
        for _ in 0..10 { closes.push(val); }

        // 11-15: Up streak (+1, +2, +3, +4, +5)
        for _ in 0..5 { val += 1.0; closes.push(val); }

        // 16-20: Down streak (-1...-5)
        for _ in 0..5 { val -= 1.0; closes.push(val); }

        // Fill up to 120 for rank lookback (100)
        for i in 0..100 {
            if i % 2 == 0 { val += 2.0; } else { val -= 1.5; }
            closes.push(val);
        }

        let df = df!("close" => closes)?;

        // Calculate
        let rsi_period = 3;
        let streak_period = 2;
        let rank_lookback = 10; // Shorten for test

        let res = calculate(&df, rsi_period, streak_period, rank_lookback)?;
        let arr = res.f64()?;

        assert_eq!(arr.len(), df.height());

        // Check validity
        // First few should be None (max lookback is 10, or streak_period+1=3, or rsi_period+1=4)
        // Lookback 10 means index 10 (11th element) is first potential valid rank?
        // Rank loop: if i < rank_lookback (10) -> push None.
        // So indices 0..9 are None. Index 10 is first valid.

        assert!(arr.get(9).is_none());
        assert!(arr.get(110).is_some());

        // Verify range 0-100
        for i in 10..arr.len() {
            if let Some(v) = arr.get(i) {
                assert!(v >= 0.0 && v <= 100.0, "CRSI {} out of bounds at {}", v, i);
            }
        }

        Ok(())
    }

    #[test]
    fn test_short_data() -> Result<()> {
        let df = df!("close" => &[100.0, 101.0, 102.0])?;
        // Lookback 10 > len 3
        let res = calculate(&df, 3, 2, 10)?;
        let arr = res.f64()?;
        assert!(arr.get(0).is_none());
        assert!(arr.get(2).is_none());
        Ok(())
    }
}
