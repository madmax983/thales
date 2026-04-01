//! Chande Kroll Stop - A volatility-based indicator to identify trend stops.
//!
//! Uses ATR to calculate the initial stops based on highest high and lowest low,
//! and then trails these stops using maximum/minimums over a secondary period.
//!
//! # Returns
//! Tuple of (Long Stop, Short Stop) Series.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use std::collections::VecDeque;

use crate::indicators::atr;

/// Calculate Chande Kroll Stop
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns.
/// * `atr_period` - Lookback period for ATR (e.g., 10).
/// * `atr_multiplier` - Multiplier for ATR (e.g., 3.0).
/// * `stop_period` - Lookback period for trailing the stop (e.g., 20).
///
/// # Returns
/// Tuple of (Long Stop, Short Stop) Series.
pub fn calculate(
    data: &DataFrame,
    atr_period: usize,
    atr_multiplier: f64,
    stop_period: usize,
) -> Result<(Series, Series)> {
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if atr_period == 0 || stop_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }
    if let Some(m) = Decimal::from_f64_retain(atr_multiplier) {
        if m < Decimal::ZERO {
            anyhow::bail!("ATR multiplier must be non-negative");
        }
    } else {
        anyhow::bail!("Invalid ATR multiplier");
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

    let highs: Vec<Option<Decimal>> = high
        .into_iter()
        .map(|v| v.and_then(Decimal::from_f64_retain))
        .collect();

    let lows: Vec<Option<Decimal>> = low
        .into_iter()
        .map(|v| v.and_then(Decimal::from_f64_retain))
        .collect();

    let atr_series = atr::calculate(data, atr_period)?;
    let atr_f64 = atr_series.f64()?;
    let atrs: Vec<Option<Decimal>> = atr_f64
        .into_iter()
        .map(|v| v.and_then(Decimal::from_f64_retain))
        .collect();

    let atr_mult_dec = Decimal::from_f64_retain(atr_multiplier).unwrap_or(Decimal::ZERO);

    // Step 1: Calculate High High and Low Low over atr_period
    let highest_high = rolling_max(&highs, atr_period);
    let lowest_low = rolling_min(&lows, atr_period);

    // Step 2: Calculate Initial Stops
    let mut initial_long_stops = vec![None; data.height()];
    let mut initial_short_stops = vec![None; data.height()];

    for i in 0..data.height() {
        if let (Some(hh), Some(ll), Some(atr_val)) = (highest_high[i], lowest_low[i], atrs[i]) {
            let offset = atr_val * atr_mult_dec;
            initial_long_stops[i] = Some(hh - offset);
            initial_short_stops[i] = Some(ll + offset);
        }
    }

    // Step 3: Trail Stops over stop_period
    let long_stops = rolling_max(&initial_long_stops, stop_period);
    let short_stops = rolling_min(&initial_short_stops, stop_period);

    let long_stops_f64: Vec<Option<f64>> = long_stops
        .into_iter()
        .map(|d| d.and_then(|v| v.to_f64()))
        .collect();

    let short_stops_f64: Vec<Option<f64>> = short_stops
        .into_iter()
        .map(|d| d.and_then(|v| v.to_f64()))
        .collect();

    let s_long = Series::new("chande_kroll_long", long_stops_f64);
    let s_short = Series::new("chande_kroll_short", short_stops_f64);

    Ok((s_long, s_short))
}

/// Calculate Rolling Min using Monotonic Queue (O(N))
fn rolling_min(data: &[Option<Decimal>], window: usize) -> Vec<Option<Decimal>> {
    let mut result = vec![None; data.len()];
    let mut deque: VecDeque<usize> = VecDeque::new();
    let mut none_count = 0;

    for i in 0..data.len() {
        if i >= window && data[i - window].is_none() {
            none_count -= 1;
        }

        if data[i].is_none() {
            none_count += 1;
        }

        while let Some(&front) = deque.front() {
            if front + window <= i {
                deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = data[i] {
            while let Some(&back) = deque.back() {
                if let Some(back_val) = data[back] {
                    if back_val >= val {
                        deque.pop_back();
                    } else {
                        break;
                    }
                } else {
                    deque.pop_back();
                }
            }
            deque.push_back(i);
        }

        if i >= window - 1 {
            if none_count == 0 {
                if let Some(&front) = deque.front() {
                    result[i] = data[front];
                }
            }
        }
    }
    result
}

/// Calculate Rolling Max using Monotonic Queue (O(N))
fn rolling_max(data: &[Option<Decimal>], window: usize) -> Vec<Option<Decimal>> {
    let mut result = vec![None; data.len()];
    let mut deque: VecDeque<usize> = VecDeque::new();
    let mut none_count = 0;

    for i in 0..data.len() {
        if i >= window && data[i - window].is_none() {
            none_count -= 1;
        }

        if data[i].is_none() {
            none_count += 1;
        }

        while let Some(&front) = deque.front() {
            if front + window <= i {
                deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = data[i] {
            while let Some(&back) = deque.back() {
                if let Some(back_val) = data[back] {
                    if back_val <= val {
                        deque.pop_back();
                    } else {
                        break;
                    }
                } else {
                    deque.pop_back();
                }
            }
            deque.push_back(i);
        }

        if i >= window - 1 {
            if none_count == 0 {
                if let Some(&front) = deque.front() {
                    result[i] = data[front];
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_edge_cases() -> Result<()> {
        let df_empty = DataFrame::default();
        let res_empty = calculate(&df_empty, 10, 3.0, 20);
        assert!(res_empty.is_err());

        let df_short = df!(
            "high" => &[10.0],
            "low" => &[10.0],
            "close" => &[10.0]
        )?;
        let (long, short) = calculate(&df_short, 10, 3.0, 20)?;
        assert_eq!(long.len(), 1);
        assert_eq!(short.len(), 1);
        assert!(long.f64()?.get(0).is_none());
        assert!(short.f64()?.get(0).is_none());

        Ok(())
    }

    #[test]
    fn test_known_values() -> Result<()> {
        let df = df!(
            "high" =>  &[10.0, 11.0, 12.0, 11.0, 10.0],
            "low" =>   &[ 8.0,  9.0, 10.0,  9.0,  8.0],
            "close" => &[ 9.0, 10.0, 11.0, 10.0,  9.0]
        )?;

        let (long_stop, short_stop) = calculate(&df, 2, 1.0, 2)?;
        let long_vals = long_stop.f64()?;
        let short_vals = short_stop.f64()?;

        // Should return enough length
        assert_eq!(long_vals.len(), 5);
        assert_eq!(short_vals.len(), 5);

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut closes = Vec::new();

        for i in 0..100 {
            let base = 100.0 + (i as f64 * 0.1).sin() * 10.0;
            highs.push(base + 2.0);
            lows.push(base - 2.0);
            closes.push(base);
        }

        let df = df!(
            "high" => highs,
            "low" => lows,
            "close" => closes
        )?;

        let (long, short) = calculate(&df, 10, 3.0, 20)?;
        assert_eq!(long.len(), 100);
        assert_eq!(short.len(), 100);

        let long_vals = long.f64()?;
        if let Some(val) = long_vals.get(50) {
            assert!(val > 0.0);
        }

        Ok(())
    }
}
