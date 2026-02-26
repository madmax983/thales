//! Ichimoku Cloud - A comprehensive indicator that defines support and resistance, identifies trend direction, gauges momentum, and provides trading signals.

use anyhow::{Context, Result};
use polars::prelude::*;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct IchimokuOutput {
    pub tenkan_sen: Series,
    pub kijun_sen: Series,
    pub senkou_span_a: Series,
    pub senkou_span_b: Series,
    pub chikou_span: Series,
}

/// Calculate Ichimoku Cloud components
///
/// # Arguments
/// * `high` - Series of High prices
/// * `low` - Series of Low prices
/// * `close` - Series of Close prices
/// * `tenkan_period` - Lookback for Tenkan-sen (Standard: 9)
/// * `kijun_period` - Lookback for Kijun-sen (Standard: 26)
/// * `senkou_b_period` - Lookback for Senkou Span B (Standard: 52)
/// * `displacement` - Displacement for Spans (Standard: 26)
///
/// # Returns
/// `IchimokuOutput` struct containing all component Series.
pub fn calculate(
    high: &Series,
    low: &Series,
    close: &Series,
    tenkan_period: usize,
    kijun_period: usize,
    senkou_b_period: usize,
    displacement: usize,
) -> Result<IchimokuOutput> {
    if high.len() != low.len() || high.len() != close.len() {
        anyhow::bail!("High, Low, and Close series must have the same length");
    }

    let highs_opt: Vec<Option<f64>> = high.f64()?.into_iter().collect();
    let lows_opt: Vec<Option<f64>> = low.f64()?.into_iter().collect();

    // 1. Tenkan-sen: (Max(9) + Min(9)) / 2
    let tenkan_max = rolling_max_opt(&highs_opt, tenkan_period);
    let tenkan_min = rolling_min_opt(&lows_opt, tenkan_period);
    let tenkan_vals = average_vecs(&tenkan_max, &tenkan_min);
    let s_tenkan = Series::new("tenkan_sen", tenkan_vals);

    // 2. Kijun-sen: (Max(26) + Min(26)) / 2
    let kijun_max = rolling_max_opt(&highs_opt, kijun_period);
    let kijun_min = rolling_min_opt(&lows_opt, kijun_period);
    let kijun_vals = average_vecs(&kijun_max, &kijun_min);
    let s_kijun = Series::new("kijun_sen", kijun_vals);

    // 3. Senkou Span A: (Tenkan + Kijun) / 2, Shifted forward by displacement
    // We can do arithmetic on Series
    let s_senkou_a_base = (&s_tenkan + &s_kijun)? / 2.0;
    let s_senkou_a = s_senkou_a_base.shift(displacement as i64);

    // 4. Senkou Span B: (Max(52) + Min(52)) / 2, Shifted forward by displacement
    let senkou_b_max = rolling_max_opt(&highs_opt, senkou_b_period);
    let senkou_b_min = rolling_min_opt(&lows_opt, senkou_b_period);
    let senkou_b_vals = average_vecs(&senkou_b_max, &senkou_b_min);
    let s_senkou_b = Series::new("senkou_span_b", senkou_b_vals).shift(displacement as i64);

    // 5. Chikou Span: Close shifted backward by displacement
    let s_chikou = close.shift(-(displacement as i64));

    Ok(IchimokuOutput {
        tenkan_sen: s_tenkan,
        kijun_sen: s_kijun,
        senkou_span_a: s_senkou_a,
        senkou_span_b: s_senkou_b,
        chikou_span: s_chikou,
    })
}

fn average_vecs(v1: &[Option<f64>], v2: &[Option<f64>]) -> Vec<Option<f64>> {
    v1.iter().zip(v2.iter()).map(|(a, b)| {
        match (a, b) {
            (Some(val1), Some(val2)) => Some((val1 + val2) / 2.0),
            _ => None,
        }
    }).collect()
}

// Rolling Max (O(N) using Monotonic Queue)
fn rolling_max_opt(values: &[Option<f64>], window_size: usize) -> Vec<Option<f64>> {
    if window_size == 0 { return vec![None; values.len()]; }
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

// Rolling Min (O(N) using Monotonic Queue)
fn rolling_min_opt(values: &[Option<f64>], window_size: usize) -> Vec<Option<f64>> {
    if window_size == 0 { return vec![None; values.len()]; }
    let mut result = Vec::with_capacity(values.len());
    let mut deque: VecDeque<usize> = VecDeque::new();

    for i in 0..values.len() {
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
    fn test_ichimoku_calculation() -> Result<()> {
        let values: Vec<f64> = (0..100).map(|i| 100.0 + i as f64).collect();
        let df = df!(
            "high" => values.clone(),
            "low" => values.clone(),
            "close" => values.clone()
        )?;

        let high = df.column("high")?;
        let low = df.column("low")?;
        let close = df.column("close")?;

        // Params: 9, 26, 52, 26
        let res = calculate(high, low, close, 9, 26, 52, 26)?;

        // Check lengths
        assert_eq!(res.tenkan_sen.len(), 100);
        assert_eq!(res.kijun_sen.len(), 100);
        assert_eq!(res.senkou_span_a.len(), 100);

        // Check Tenkan at index 8 (9th element)
        // Highs 0..8: 100..108. Max 108.
        // Lows 0..8: 100..108. Min 100.
        // Avg: 104.
        let tenkan = res.tenkan_sen.f64()?;
        assert_eq!(tenkan.get(8), Some(104.0));
        assert!(tenkan.get(7).is_none());

        // Check Senkou A
        // It's shifted 26. So index 0..25 should be null.
        let span_a = res.senkou_span_a.f64()?;
        assert!(span_a.get(25).is_none());
        // Index 26 should correspond to (Tenkan[0] + Kijun[0])/2.
        // But Tenkan[0] is null. So SpanA[26] should be null.
        // First valid Tenkan is at 8. First valid Kijun is at 25.
        // So first valid (Tenkan+Kijun)/2 is at 25.
        // Shifted 26 -> 25+26 = 51.
        // So Span A should be valid at 51.
        assert!(span_a.get(50).is_none());
        assert!(span_a.get(51).is_some());

        Ok(())
    }
}
