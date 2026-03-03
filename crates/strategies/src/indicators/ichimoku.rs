//! Ichimoku Cloud - A collection of technical indicators that show support and resistance, as well as momentum and trend direction.

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Calculate Ichimoku Cloud components
///
/// # Arguments
/// * `data` - DataFrame with "high", "low", "close" columns.
/// * `tenkan_period` - Lookback period for Tenkan-sen (Conversion Line). Default: 9.
/// * `kijun_period` - Lookback period for Kijun-sen (Base Line). Default: 26.
/// * `senkou_span_b_period` - Lookback period for Senkou Span B (Leading Span B). Default: 52.
/// * `senkou_span_offset` - Offset for Senkou Spans A and B (projected forward). Default: 26.
/// * `chikou_span_offset` - Offset for Chikou Span (lagging span). Default: 26.
///
/// # Returns
/// Tuple of 5 Series:
/// (Tenkan-sen, Kijun-sen, Senkou Span A, Senkou Span B, Chikou Span)
///
/// # Example
/// ```rust
/// use strategies::indicators::ichimoku;
/// use polars::prelude::*;
///
/// // Assuming df is a DataFrame with "high", "low", "close" columns
/// let tenkan = 9;
/// let kijun = 26;
/// let span_b = 52;
/// let offset = 26;
/// let chikou_offset = 26;
/// // let (tenkan, kijun, span_a, span_b, chikou) = ichimoku::calculate(&df, tenkan, kijun, span_b, offset, chikou_offset)?;
/// ```
pub fn calculate(
    data: &DataFrame,
    tenkan_period: usize,
    kijun_period: usize,
    senkou_span_b_period: usize,
    senkou_span_offset: usize,
    chikou_span_offset: usize,
) -> Result<(Series, Series, Series, Series, Series)> {
    // Validate inputs
    if data.height() == 0 {
        anyhow::bail!("Data cannot be empty");
    }
    if tenkan_period == 0 || kijun_period == 0 || senkou_span_b_period == 0 {
        anyhow::bail!("Periods must be greater than 0");
    }

    // Get columns
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

    // Helper closure to calculate (Max + Min) / 2
    let calc_midpoint =
        |highs: &[Option<f64>], lows: &[Option<f64>], period: usize| -> Result<Vec<Option<f64>>> {
            let max_vals = rolling_max_opt(highs, period);
            let min_vals = rolling_min_opt(lows, period);
            let mut result = Vec::with_capacity(highs.len());
            let two = Decimal::from(2);

            for i in 0..highs.len() {
                match (max_vals[i], min_vals[i]) {
                    (Some(h), Some(l)) => {
                        if let (Some(h_dec), Some(l_dec)) =
                            (Decimal::from_f64_retain(h), Decimal::from_f64_retain(l))
                        {
                            let mid = (h_dec + l_dec) / two;
                            result.push(mid.to_f64());
                        } else {
                            result.push(None);
                        }
                    }
                    _ => result.push(None),
                }
            }
            Ok(result)
        };

    let highs_vec: Vec<Option<f64>> = high.into_iter().collect();
    let lows_vec: Vec<Option<f64>> = low.into_iter().collect();

    // 1. Calculate Tenkan-sen
    let tenkan_vals = calc_midpoint(&highs_vec, &lows_vec, tenkan_period)?;
    let tenkan_series = Series::new("tenkan_sen", &tenkan_vals);

    // 2. Calculate Kijun-sen
    let kijun_vals = calc_midpoint(&highs_vec, &lows_vec, kijun_period)?;
    let kijun_series = Series::new("kijun_sen", &kijun_vals);

    // 3. Calculate Senkou Span A: (Tenkan + Kijun) / 2 shifted forward
    let mut span_a_vals = Vec::with_capacity(data.height());
    let two = Decimal::from(2);

    // We calculate Span A based on Tenkan and Kijun *before* shifting.
    // The value for Span A at time T is derived from Tenkan and Kijun at time T-offset.
    // So if we iterate, span_a[i] = (tenkan[i] + kijun[i]) / 2.
    // Then we shift the whole series forward by `senkou_span_offset`.

    for i in 0..data.height() {
        match (tenkan_vals[i], kijun_vals[i]) {
            (Some(t), Some(k)) => {
                if let (Some(t_dec), Some(k_dec)) =
                    (Decimal::from_f64_retain(t), Decimal::from_f64_retain(k))
                {
                    let avg = (t_dec + k_dec) / two;
                    span_a_vals.push(avg.to_f64());
                } else {
                    span_a_vals.push(None);
                }
            }
            _ => span_a_vals.push(None),
        }
    }
    let span_a_raw = Series::new("senkou_span_a_raw", &span_a_vals);
    let mut span_a_shifted = span_a_raw.shift(senkou_span_offset as i64);
    let _ = span_a_shifted.rename("senkou_span_a");

    // 4. Calculate Senkou Span B: (Max + Min) / 2 of span_b_period, shifted forward
    let span_b_raw_vals = calc_midpoint(&highs_vec, &lows_vec, senkou_span_b_period)?;
    let span_b_raw = Series::new("senkou_span_b_raw", &span_b_raw_vals);
    let mut span_b_shifted = span_b_raw.shift(senkou_span_offset as i64);
    let _ = span_b_shifted.rename("senkou_span_b");

    // 5. Calculate Chikou Span: Close shifted backward
    // Chikou Span is plotted 26 periods behind. So today's close is plotted at T-26.
    // In a DataFrame aligned to "Today" (T), the value at T-26 should show Close[T].
    // Wait. "Chikou Span is the Closing price plotted 26 periods behind."
    // If I look at index `i`, I should see the Close price from `i+26`?
    // If so, `chikou[i] = close[i+26]`.
    // This is a backward shift (negative shift).
    // `close.shift(-26)`: value at `i` becomes value at `i+26`. Correct.
    let chikou_shifted_ca = close.shift(-(chikou_span_offset as i64));
    let chikou_series = Series::new("chikou_span", chikou_shifted_ca);

    Ok((
        tenkan_series,
        kijun_series,
        span_a_shifted,
        span_b_shifted,
        chikou_series,
    ))
}

// Helper functions for rolling calculations using Monotonic Queue (O(N))
// Adapted to handle Option<f64>
fn rolling_max_opt(values: &[Option<f64>], window_size: usize) -> Vec<Option<f64>> {
    if window_size == 0 {
        return vec![None; values.len()];
    }
    let mut result = Vec::with_capacity(values.len());
    let mut deque: VecDeque<usize> = VecDeque::new();

    for i in 0..values.len() {
        // Remove indices out of window
        while let Some(&front) = deque.front() {
            if front + window_size <= i {
                deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = values[i] {
            // Maintain decreasing order for Max
            while let Some(&back) = deque.back() {
                if let Some(back_val) = values[back] {
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

        // Result for this window
        if i >= window_size - 1 {
            if let Some(&front) = deque.front() {
                result.push(values[front]);
            } else {
                result.push(None);
            }
        } else {
            result.push(None);
        }
    }
    result
}

fn rolling_min_opt(values: &[Option<f64>], window_size: usize) -> Vec<Option<f64>> {
    if window_size == 0 {
        return vec![None; values.len()];
    }
    let mut result = Vec::with_capacity(values.len());
    let mut deque: VecDeque<usize> = VecDeque::new();

    for i in 0..values.len() {
        // Remove indices out of window
        while let Some(&front) = deque.front() {
            if front + window_size <= i {
                deque.pop_front();
            } else {
                break;
            }
        }

        if let Some(val) = values[i] {
            // Maintain increasing order for Min
            while let Some(&back) = deque.back() {
                if let Some(back_val) = values[back] {
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

        if i >= window_size - 1 {
            if let Some(&front) = deque.front() {
                result.push(values[front]);
            } else {
                result.push(None);
            }
        } else {
            result.push(None);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[test]
    fn test_known_values() -> Result<()> {
        // Period: 9, 26, 52. Offset: 26.
        // We need at least 52 + 26 = 78 points to verify full behavior?
        // Or simpler: Period 3, 5, 7. Offset 2.

        // Tenkan (3): Max/Min of last 3.
        // Kijun (5): Max/Min of last 5.
        // Span A: (Tenkan + Kijun)/2 shifted 2.
        // Span B (7): Max/Min of last 7 shifted 2.
        // Chikou: Close shifted -2.

        let n = 20;
        let mut closes = Vec::new();
        let mut highs = Vec::new();
        let mut lows = Vec::new();

        for i in 0..n {
            let val = (i as f64) * 10.0;
            closes.push(val);
            highs.push(val + 5.0);
            lows.push(val - 5.0);
        }

        let df = df!(
            "close" => &closes,
            "high" => &highs,
            "low" => &lows
        )?;

        let (tenkan, kijun, span_a, span_b, chikou) = calculate(&df, 3, 5, 7, 2, 2)?;

        // Check Tenkan at index 2 (window 0-2)
        // Highs: 5, 15, 25. Max 25.
        // Lows: -5, 5, 15. Min -5.
        // Tenkan = (25 + -5) / 2 = 10.
        assert_eq!(tenkan.f64()?.get(2), Some(10.0));

        // Check Kijun at index 4 (window 0-4)
        // Highs: 5, 15, 25, 35, 45. Max 45.
        // Lows: -5, 5, 15, 25, 35. Min -5.
        // Kijun = (45 + -5) / 2 = 20.
        assert_eq!(kijun.f64()?.get(4), Some(20.0));

        // Check Span A at index 4+2 = 6.
        // It comes from index 4.
        // Tenkan at 4: Highs[2-4]: 25, 35, 45. Max 45. Lows 15, 25, 35. Min 15. Avg = (45+15)/2 = 30.
        // Kijun at 4: Avg 20.
        // Span A Raw at 4: (30 + 20) / 2 = 25.
        // Span A Shifted at 6 should be 25.
        assert_eq!(span_a.f64()?.get(6), Some(25.0));

        // Check Span B at index 6+2 = 8.
        // Comes from index 6.
        // Period 7 (0-6).
        // Max High 65. Min Low -5. Avg (65 - 5)/2 = 30.
        // Span B Shifted at 8 should be 30.
        assert_eq!(span_b.f64()?.get(8), Some(30.0));

        // Check Chikou at index 0.
        // Shifted -2. So Chikou[0] = Close[0+2] = Close[2] = 20.
        assert_eq!(chikou.f64()?.get(0), Some(20.0));

        // Check Chikou at end (index 18).
        // Chikou[18] = Close[20] -> None (out of bounds).
        assert!(chikou.f64()?.get(18).is_none());

        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Empty
        let df_empty = DataFrame::default();
        assert!(calculate(&df_empty, 9, 26, 52, 26, 26).is_err());

        // Short data
        let df_short = df!(
            "close" => &[10.0, 11.0],
            "high" => &[12.0, 13.0],
            "low" => &[9.0, 10.0]
        )?;
        // Period 9. Result should be all None for Tenkan/Kijun/SpanA/SpanB (due to warmup or shift)
        // Chikou should be partially valid (shift -26 -> None, shift -1 -> valid).

        let (t, k, sa, sb, c) = calculate(&df_short, 9, 26, 52, 26, 26)?;

        // With only 2 points, rolling max(9) is None.
        assert!(t.f64()?.get(0).is_none());
        assert!(t.f64()?.get(1).is_none());

        Ok(())
    }

    #[test]
    fn test_realistic_data() -> Result<()> {
        // Generate a sine wave price to test movement
        // Price oscillates between 90 and 110.
        // We expect Cloud to follow price with lag.
        // Chikou should be price shifted back.

        let n = 100;
        let mut closes = Vec::with_capacity(n);
        let mut highs = Vec::with_capacity(n);
        let mut lows = Vec::with_capacity(n);

        for i in 0..n {
            let val = 100.0 + (i as f64 * 0.1).sin() * 10.0;
            closes.push(val);
            highs.push(val + 2.0);
            lows.push(val - 2.0);
        }

        let df = df!(
            "close" => &closes,
            "high" => &highs,
            "low" => &lows
        )?;

        // Standard parameters: 9, 26, 52, 26, 26
        let (tenkan, kijun, span_a, span_b, chikou) = calculate(&df, 9, 26, 52, 26, 26)?;

        // Verify basic properties
        assert_eq!(tenkan.len(), n);
        assert_eq!(kijun.len(), n);
        assert_eq!(span_a.len(), n);
        assert_eq!(span_b.len(), n);
        assert_eq!(chikou.len(), n);

        // Check NaNs at start
        // Tenkan needs 9 periods (index 8 valid)
        assert!(tenkan.f64()?.get(7).is_none());
        assert!(tenkan.f64()?.get(8).is_some());

        // Kijun needs 26 periods (index 25 valid)
        assert!(kijun.f64()?.get(24).is_none());
        assert!(kijun.f64()?.get(25).is_some());

        // Span A shifted 26. Valid from index 8 + 26 = 34?
        // Span A = (Tenkan + Kijun)/2 shifted 26.
        // Needs Kijun valid (index 25). So Span A raw valid at 25.
        // Shifted 26 -> Valid at 25 + 26 = 51.
        assert!(span_a.f64()?.get(50).is_none());
        assert!(span_a.f64()?.get(51).is_some());

        // Span B shifted 26.
        // Needs 52 periods (index 51 valid). Shifted 26 -> Valid at 51 + 26 = 77.
        assert!(span_b.f64()?.get(76).is_none());
        assert!(span_b.f64()?.get(77).is_some());

        // Chikou shifted -26.
        // First value valid (Close[26]). Last 26 invalid.
        assert!(chikou.f64()?.get(0).is_some());
        assert!(chikou.f64()?.get(n - 27).is_some());
        assert!(chikou.f64()?.get(n - 26).is_none());

        Ok(())
    }
}
