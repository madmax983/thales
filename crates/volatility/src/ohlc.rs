//! OHLC volatility estimators (Sinclair framework, Part 1).
//!
//! Close-to-close variance wastes the information in the trading range.
//! These range-based estimators extract more signal per bar, in increasing
//! order of efficiency and robustness:
//!
//! - [`parkinson`]: high-low range only; ~5x the efficiency of
//!   close-to-close when drift is negligible.
//! - [`garman_klass`]: adds the open-to-close move; best under small drift.
//! - [`rogers_satchell`]: drift-independent; the workhorse when trend is
//!   present.
//! - [`yang_zhang`]: combines overnight, open-to-close, and Rogers–Satchell
//!   terms; handles drift *and* overnight jumps. The default for daily bars.
//!
//! All return **per-period variance**. Annualize with
//! `variance * periods_per_year` then sqrt. All are pure functions of the
//! input bars: deterministic, no I/O. Fail closed on bad input.
//!
//! Combine several estimators × lookbacks with [`combine_forecasts`]
//! (inverse-MSE weighting) rather than trusting any single one.

use super::FitError;

/// One OHLC bar. Map from `contracts::Bar` at the call site; this crate
/// stays dependency-free.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ohlc {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

fn validate(bars: &[Ohlc], min_bars: usize) -> Result<(), FitError> {
    if bars.len() < min_bars {
        return Err(FitError::InsufficientData {
            min: min_bars,
            got: bars.len(),
        });
    }
    for (i, b) in bars.iter().enumerate() {
        for v in [b.open, b.high, b.low, b.close] {
            if !v.is_finite() || v <= 0.0 {
                return Err(FitError::NonFiniteInput(i));
            }
        }
        if b.high < b.low {
            return Err(FitError::NonFiniteInput(i));
        }
    }
    Ok(())
}

/// Parkinson (1980): `mean( (ln(H/L))² / (4·ln 2) )`.
pub fn parkinson(bars: &[Ohlc]) -> Result<f64, FitError> {
    validate(bars, 2)?;
    let denom = 4.0 * std::f64::consts::LN_2;
    let sum: f64 = bars
        .iter()
        .map(|b| {
            let r = (b.high / b.low).ln();
            r * r / denom
        })
        .sum();
    Ok(sum / bars.len() as f64)
}

/// Garman–Klass (1980):
/// `mean( 0.5·(ln(H/L))² − (2·ln 2 − 1)·(ln(C/O))² )`.
pub fn garman_klass(bars: &[Ohlc]) -> Result<f64, FitError> {
    validate(bars, 2)?;
    let k = 2.0 * std::f64::consts::LN_2 - 1.0;
    let sum: f64 = bars
        .iter()
        .map(|b| {
            let hl = (b.high / b.low).ln();
            let co = (b.close / b.open).ln();
            0.5 * hl * hl - k * co * co
        })
        .sum();
    Ok((sum / bars.len() as f64).max(0.0))
}

/// Rogers–Satchell (1991), drift-independent:
/// `mean( ln(H/C)·ln(H/O) + ln(L/C)·ln(L/O) )`.
pub fn rogers_satchell(bars: &[Ohlc]) -> Result<f64, FitError> {
    validate(bars, 2)?;
    let sum: f64 = bars
        .iter()
        .map(|b| {
            (b.high / b.close).ln() * (b.high / b.open).ln()
                + (b.low / b.close).ln() * (b.low / b.open).ln()
        })
        .sum();
    Ok((sum / bars.len() as f64).max(0.0))
}

/// Yang–Zhang (2000):
/// `σ² = σ_overnight² + k·σ_openclose² + (1−k)·σ_rs²`,
/// with `k = 0.34 / (1.34 + (n+1)/(n−1))`.
///
/// Handles both drift and overnight jumps. Needs ≥ 2 bars (overnight
/// returns are close-to-open).
pub fn yang_zhang(bars: &[Ohlc]) -> Result<f64, FitError> {
    validate(bars, 2)?;
    let n = bars.len() as f64;

    // Overnight: ln(O_i / C_{i-1}), mean-adjusted sample variance.
    let overnight: Vec<f64> = bars
        .windows(2)
        .map(|w| (w[1].open / w[0].close).ln())
        .collect();
    let mean_o = overnight.iter().sum::<f64>() / overnight.len() as f64;
    let var_o = overnight
        .iter()
        .map(|&x| (x - mean_o) * (x - mean_o))
        .sum::<f64>()
        / overnight.len() as f64;

    // Open-to-close: ln(C_i / O_i), mean-adjusted.
    let oc: Vec<f64> = bars.iter().map(|b| (b.close / b.open).ln()).collect();
    let mean_c = oc.iter().sum::<f64>() / oc.len() as f64;
    let var_c = oc.iter().map(|&x| (x - mean_c) * (x - mean_c)).sum::<f64>() / oc.len() as f64;

    // Rogers–Satchell component.
    let var_rs = rogers_satchell(bars)?;

    let k = 0.34 / (1.34 + (n + 1.0) / (n - 1.0));
    Ok((var_o + k * var_c + (1.0 - k) * var_rs).max(0.0))
}

/// Combines forecasts with inverse-MSE weights: each forecast is weighted
/// by `1/mse`, normalized. The standard Sinclair practice — a weighted
/// average of several estimators × lookbacks beats any single "best" model
/// out of sample.
///
/// `forecasts` is `(forecast, historical_mse)` pairs. Returns `None` when
/// empty or when any MSE is non-positive (fail closed).
pub fn combine_forecasts(forecasts: &[(f64, f64)]) -> Option<f64> {
    if forecasts.is_empty() {
        return None;
    }
    let mut inv_sum = 0.0;
    for &(_, mse) in forecasts {
        if !mse.is_finite() || mse <= 0.0 {
            return None;
        }
        inv_sum += 1.0 / mse;
    }
    let combined = forecasts
        .iter()
        .map(|&(f, mse)| f * (1.0 / mse) / inv_sum)
        .sum();
    Some(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bars() -> Vec<Ohlc> {
        vec![
            Ohlc {
                open: 100.0,
                high: 102.0,
                low: 98.0,
                close: 101.0,
            },
            Ohlc {
                open: 101.0,
                high: 103.0,
                low: 99.0,
                close: 102.0,
            },
            Ohlc {
                open: 102.0,
                high: 104.0,
                low: 100.0,
                close: 103.0,
            },
        ]
    }

    fn flat() -> Vec<Ohlc> {
        vec![
            Ohlc {
                open: 100.0,
                high: 100.0,
                low: 100.0,
                close: 100.0,
            },
            Ohlc {
                open: 100.0,
                high: 100.0,
                low: 100.0,
                close: 100.0,
            },
        ]
    }

    #[test]
    fn flat_series_has_zero_variance() {
        for est in [
            parkinson(&flat()),
            garman_klass(&flat()),
            rogers_satchell(&flat()),
            yang_zhang(&flat()),
        ] {
            assert_eq!(est.unwrap(), 0.0);
        }
    }

    #[test]
    fn parkinson_matches_hand_computation() {
        // mean of (ln(H/L))^2 / (4 ln 2) over the three bars.
        let v = parkinson(&bars()).unwrap();
        assert!((v - 0.0005659663).abs() < 1e-9, "parkinson {v}");
    }

    #[test]
    fn garman_klass_matches_hand_computation() {
        let v = garman_klass(&bars()).unwrap();
        assert!((v - 0.0007470918).abs() < 1e-9, "gk {v}");
    }

    #[test]
    fn rogers_satchell_matches_hand_computation() {
        let v = rogers_satchell(&bars()).unwrap();
        assert!((v - 0.0007885383).abs() < 1e-9, "rs {v}");
    }

    #[test]
    fn yang_zhang_matches_hand_computation() {
        let v = yang_zhang(&bars()).unwrap();
        assert!((v - 0.0007082686).abs() < 1e-9, "yz {v}");
    }

    #[test]
    fn yang_zhang_needs_two_bars() {
        let one = vec![Ohlc {
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.5,
        }];
        assert!(matches!(
            yang_zhang(&one),
            Err(FitError::InsufficientData { min: 2, got: 1 })
        ));
    }

    #[test]
    fn bad_input_fails_closed() {
        let mut bad = bars();
        bad[1].high = -5.0;
        assert!(parkinson(&bad).is_err());
        let mut bad2 = bars();
        bad2[0].low = 200.0; // high < low
        assert!(garman_klass(&bad2).is_err());
    }

    #[test]
    fn combine_weights_by_inverse_mse() {
        // Equal MSE -> simple average.
        let c = combine_forecasts(&[(0.2, 1.0), (0.4, 1.0)]).unwrap();
        assert!((c - 0.3).abs() < 1e-12, "combined {c}");
        // Lower MSE dominates: weights 4/5 vs 1/5.
        let c = combine_forecasts(&[(0.2, 1.0), (0.4, 4.0)]).unwrap();
        assert!((c - 0.24).abs() < 1e-12, "combined {c}");
        // Degenerate inputs fail closed.
        assert_eq!(combine_forecasts(&[]), None);
        assert_eq!(combine_forecasts(&[(0.2, 0.0), (0.4, 1.0)]), None);
        assert_eq!(combine_forecasts(&[(0.2, -1.0), (0.4, 1.0)]), None);
    }
}
