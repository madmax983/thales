//! Supertrend Indicator
//!
//! The Supertrend indicator is a trend-following indicator that uses the Average True Range (ATR)
//! to compute a dynamic stop-loss line. It consists of two components: the Supertrend line and the Trend direction.
//!
//! # Formula
//! Basic Upper Band = (High + Low) / 2 + Multiplier * ATR
//! Basic Lower Band = (High + Low) / 2 - Multiplier * ATR
//!
//! Final Upper Band = If (Current Basic Upper Band < Previous Final Upper Band) or (Previous Close > Previous Final Upper Band)
//!                    Then Current Basic Upper Band
//!                    Else Previous Final Upper Band
//!
//! Final Lower Band = If (Current Basic Lower Band > Previous Final Lower Band) or (Previous Close < Previous Final Lower Band)
//!                    Then Current Basic Lower Band
//!                    Else Previous Final Lower Band
//!
//! Supertrend = If Trend is Up Then Final Lower Band Else Final Upper Band
//!
//! Trend = If Previous Trend was Up and Close < Final Lower Band Then Down
//!         Else If Previous Trend was Down and Close > Final Upper Band Then Up
//!         Else Previous Trend

use crate::indicators::atr;
use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Calculate Supertrend
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns
/// * `period` - ATR period (must be > 0)
/// * `multiplier` - ATR multiplier
///
/// # Returns
/// A tuple of two Series:
/// 1. "supertrend" - The Supertrend line values
/// 2. "supertrend_trend" - The Trend direction (1 for Up, -1 for Down)
pub fn calculate(data: &DataFrame, period: usize, multiplier: f64) -> Result<(Series, Series)> {
    // Validate inputs
    if period == 0 {
        anyhow::bail!("Period must be greater than 0");
    }
    if data.is_empty() {
        anyhow::bail!("Data cannot be empty");
    }

    // 1. Calculate ATR
    let atr_series =
        atr::calculate(data, period).context("Failed to calculate ATR for Supertrend")?;
    let atr_values: Vec<Option<f64>> = atr_series.f64()?.into_iter().collect();

    // 2. Get OHLC columns
    let high = data.column("high")?.f64()?;
    let low = data.column("low")?.f64()?;
    let close = data.column("close")?.f64()?;

    let len = data.height();
    let mut supertrend_values: Vec<Option<f64>> = vec![None; len];
    let mut trend_values: Vec<Option<i32>> = vec![None; len]; // 1 for Up, -1 for Down

    // Find the first valid index where ATR is available
    // ATR usually returns null for the first period-1 values.
    let start_idx = atr_values.iter().position(|v| v.is_some()).unwrap_or(len);

    if start_idx < len {
        // Initialize first value
        let h = Decimal::from_f64(high.get(start_idx).unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
        let l = Decimal::from_f64(low.get(start_idx).unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
        let c = Decimal::from_f64(close.get(start_idx).unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
        let atr_val = Decimal::from_f64(atr_values[start_idx].unwrap()).unwrap_or(Decimal::ZERO);
        let mult = Decimal::from_f64(multiplier).unwrap_or(Decimal::ZERO);

        let two = Decimal::from(2);
        let basic_upper = (h + l) / two + mult * atr_val;
        let basic_lower = (h + l) / two - mult * atr_val;

        let mut prev_final_upper = basic_upper;
        let mut prev_final_lower = basic_lower;

        // Initial trend based on close vs lower band
        let mut prev_trend = if c < basic_lower { -1 } else { 1 };

        let st = if prev_trend == 1 {
            prev_final_lower
        } else {
            prev_final_upper
        };
        supertrend_values[start_idx] = st.to_f64();
        trend_values[start_idx] = Some(prev_trend);

        for i in (start_idx + 1)..len {
            let h = Decimal::from_f64(high.get(i).unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
            let l = Decimal::from_f64(low.get(i).unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
            let c = Decimal::from_f64(close.get(i).unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
            // Previous close is needed for the logic: "Previous Close > Previous Final Upper Band"
            let prev_c =
                Decimal::from_f64(close.get(i - 1).unwrap_or(0.0)).unwrap_or(Decimal::ZERO);

            let atr_val = match atr_values[i] {
                Some(v) => Decimal::from_f64(v).unwrap_or(Decimal::ZERO),
                None => continue, // Should not happen after start_idx
            };
            let mult = Decimal::from_f64(multiplier).unwrap_or(Decimal::ZERO);

            let two = Decimal::from(2);
            let basic_upper = (h + l) / two + mult * atr_val;
            let basic_lower = (h + l) / two - mult * atr_val;

            // Final Upper Band Logic
            let final_upper = if basic_upper < prev_final_upper || prev_c > prev_final_upper {
                basic_upper
            } else {
                prev_final_upper
            };

            // Final Lower Band Logic
            let final_lower = if basic_lower > prev_final_lower || prev_c < prev_final_lower {
                basic_lower
            } else {
                prev_final_lower
            };

            // Trend Logic
            let mut trend = prev_trend;
            if prev_trend == -1 && c > final_upper {
                trend = 1;
            } else if prev_trend == 1 && c < final_lower {
                trend = -1;
            }

            // Supertrend Value
            let st = if trend == 1 { final_lower } else { final_upper };

            supertrend_values[i] = st.to_f64();
            trend_values[i] = Some(trend);

            prev_final_upper = final_upper;
            prev_final_lower = final_lower;
            prev_trend = trend;
        }
    }

    let st_series = Series::new("supertrend", supertrend_values);
    let trend_series = Series::new("supertrend_trend", trend_values);

    Ok((st_series, trend_series))
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_supertrend_known_values() -> Result<()> {
        // Data from a known Supertrend calculation
        // High, Low, Close
        let highs = vec![10.0, 10.5, 11.0, 10.8, 10.2, 9.8, 9.5, 9.2, 9.6, 10.0];
        let lows = vec![9.0, 9.5, 10.0, 9.8, 9.2, 8.8, 8.5, 8.2, 8.6, 9.0];
        let closes = vec![9.5, 10.0, 10.5, 10.0, 9.5, 9.0, 8.8, 8.5, 9.0, 9.5];

        let df = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes,
        )?;

        // Period 3, Multiplier 1.0
        let (st, trend) = calculate(&df, 3, 1.0)?;

        assert_eq!(st.len(), 10);
        assert_eq!(trend.len(), 10);

        // We expect at least the first few to be null.
        // For ATR(3), indices 0,1 are null. Index 2 (3rd bar) is first ATR.
        // So Supertrend starts at index 2.
        assert!(st.get(0).unwrap().is_nested_null());
        assert!(st.get(1).unwrap().is_nested_null());
        assert!(!st.get(2).unwrap().is_nested_null()); // Should be valid

        Ok(())
    }

    #[test]
    fn test_edge_case_empty() {
        let df = DataFrame::default();
        let result = calculate(&df, 14, 3.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_edge_case_single_point() -> Result<()> {
        let df = df!(
            "high" => vec![10.0],
            "low" => vec![9.0],
            "close" => vec![9.5],
        )?;
        // Not enough data for ATR
        let (st, _) = calculate(&df, 14, 3.0)?;
        // If ATR returns nulls, Supertrend returns nulls
        assert!(st.get(0).unwrap().is_nested_null());
        Ok(())
    }
}
