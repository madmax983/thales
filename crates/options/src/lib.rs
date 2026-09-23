//! Options pricing for Thales (SPEC-006 v0).
//!
//! Pure, deterministic, no I/O:
//! - [`black_scholes`]: European option price plus Greeks (delta, gamma,
//!   theta per year, vega per 1.0 vol point).
//! - [`norm_cdf`]: standard normal CDF via erf (Abramowitz & Stegun 7.1.26,
//!   |error| <= 1.5e-7 — plenty for pricing).
//! - [`iv_rank`]: conventional IV rank — (current − min)/(max − min) × 100.
//! - [`iv_percentile`]: percentile rank of current IV against a trailing history.
//! - [`edge`]: forecast-vs-implied edge, variance risk premium, and the
//!   delta-hedged gamma/theta P&L identity (Sinclair Part 2).
//!
//! Conventions: `t_years` uses ACT/365; `rate` is the continuously-compounded
//! risk-free rate. Degenerate inputs fail closed to intrinsic value with zero
//! Greeks (`t_years <= 0`); non-finite inputs are a [`PricingError`].

pub mod edge;

use thiserror::Error;

/// Errors from the options pricer.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum PricingError {
    /// A price, strike, time, rate, or vol input was NaN or infinite.
    #[error("non-finite input: {0}")]
    NonFiniteInput(&'static str),
    /// Volatility was negative.
    #[error("negative volatility: {0}")]
    NegativeVolatility(f64),
}

/// Call or put.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
    Call,
    Put,
}

/// Price plus Greeks from [`black_scholes`].
///
/// `theta` is per **year** (divide by 365 for per-day); `vega` is per 1.0 of
/// volatility (divide by 100 for per-vol-point).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Greeks {
    pub price: f64,
    pub delta: f64,
    pub gamma: f64,
    pub theta: f64,
    pub vega: f64,
}

/// Error function via Abramowitz & Stegun 7.1.26 (|error| <= 1.5e-7).
fn erf_as(x: f64) -> f64 {
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let ax = x.abs();
    let t = 1.0 / (1.0 + p * ax);
    let poly = ((((a5 * t + a4) * t + a3) * t + a2) * t + a1) * t;
    sign * (1.0 - poly * (-ax * ax).exp())
}

/// Standard normal CDF: Φ(x) = 0.5 * (1 + erf(x / √2)).
pub fn norm_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf_as(x / std::f64::consts::SQRT_2))
}

/// Standard normal PDF.
pub fn norm_pdf(x: f64) -> f64 {
    (-x * x / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

/// European Black-Scholes price and Greeks.
///
/// Fails closed: `t_years <= 0` returns intrinsic value with zero Greeks;
/// non-finite inputs or negative vol are [`PricingError`]; `iv == 0` prices
/// the discounted intrinsic (the zero-vol limit).
pub fn black_scholes(
    option_type: OptionType,
    spot: f64,
    strike: f64,
    t_years: f64,
    rate: f64,
    iv: f64,
) -> Result<Greeks, PricingError> {
    for (name, v) in [
        ("spot", spot),
        ("strike", strike),
        ("t_years", t_years),
        ("rate", rate),
        ("iv", iv),
    ] {
        if !v.is_finite() {
            return Err(PricingError::NonFiniteInput(name));
        }
    }
    if iv < 0.0 {
        return Err(PricingError::NegativeVolatility(iv));
    }
    if spot <= 0.0 || strike <= 0.0 {
        return Err(PricingError::NonFiniteInput("spot/strike must be positive"));
    }

    // Expired: intrinsic value, no Greeks.
    if t_years <= 0.0 {
        let intrinsic = match option_type {
            OptionType::Call => (spot - strike).max(0.0),
            OptionType::Put => (strike - spot).max(0.0),
        };
        return Ok(Greeks {
            price: intrinsic,
            delta: 0.0,
            gamma: 0.0,
            theta: 0.0,
            vega: 0.0,
        });
    }

    // Zero vol: the sigma -> 0 limit is discounted intrinsic,
    // max(S - K*e^(-rT), 0) for calls. (Discount the strike, not the spread.)
    if iv == 0.0 {
        let disc = (-rate * t_years).exp();
        let (price, delta) = match option_type {
            OptionType::Call => {
                let p = (spot - strike * disc).max(0.0);
                (p, if spot > strike * disc { 1.0 } else { 0.0 })
            }
            OptionType::Put => {
                let p = (strike * disc - spot).max(0.0);
                (p, if spot < strike * disc { -1.0 } else { 0.0 })
            }
        };
        return Ok(Greeks {
            price,
            delta,
            gamma: 0.0,
            theta: 0.0,
            vega: 0.0,
        });
    }

    let sqrt_t = t_years.sqrt();
    let d1 = (spot / strike).ln() + (rate + 0.5 * iv * iv) * t_years;
    let d1 = d1 / (iv * sqrt_t);
    let d2 = d1 - iv * sqrt_t;
    let disc = (-rate * t_years).exp();

    let nd1 = norm_cdf(d1);
    let nd2 = norm_cdf(d2);
    let pdf_d1 = norm_pdf(d1);

    let (price, delta) = match option_type {
        OptionType::Call => (spot * nd1 - strike * disc * nd2, nd1),
        OptionType::Put => (
            strike * disc * norm_cdf(-d2) - spot * norm_cdf(-d1),
            nd1 - 1.0,
        ),
    };
    let gamma = pdf_d1 / (spot * iv * sqrt_t);
    let vega = spot * pdf_d1 * sqrt_t;
    // Theta per year, standard Black-Scholes form.
    let theta_common = -(spot * pdf_d1 * iv) / (2.0 * sqrt_t);
    let theta = match option_type {
        OptionType::Call => theta_common - rate * strike * disc * nd2,
        OptionType::Put => theta_common + rate * strike * disc * norm_cdf(-d2),
    };

    Ok(Greeks {
        price,
        delta,
        gamma,
        theta,
        vega,
    })
}

/// Conventional IV rank: where `current_iv` sits between the trailing
/// `history` min and max, scaled to 0–100.
///
/// `Some(100.0)` = at the highs, `Some(0.0)` = at the lows, `None` when
/// history is empty or degenerate (flat history ranks 100 by the percentile
/// convention below; here it is undefined and returns `None`).
pub fn iv_rank(current_iv: f64, history: &[f64]) -> Option<f64> {
    if history.is_empty() || !current_iv.is_finite() {
        return None;
    }
    let min = history.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = history.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let spread = max - min;
    if !spread.is_finite() || spread <= 0.0 {
        return None;
    }
    Some(100.0 * (current_iv - min) / spread)
}

/// IV percentile: share of `history` at or below `current_iv`, scaled 0–100.
///
/// `Some(100.0)` means the current IV is the highest on record; `None` when
/// history is empty. Ties count as "at or below", so a flat history
/// percentiles at 100.
pub fn iv_percentile(current_iv: f64, history: &[f64]) -> Option<f64> {
    if history.is_empty() || !current_iv.is_finite() {
        return None;
    }
    let at_or_below = history.iter().filter(|&&h| h <= current_iv).count();
    Some(100.0 * at_or_below as f64 / history.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hull textbook values: S=42, K=40, r=10%, sigma=20%, T=0.5.
    const S: f64 = 42.0;
    const K: f64 = 40.0;
    const R: f64 = 0.10;
    const V: f64 = 0.20;
    const T: f64 = 0.5;

    #[test]
    fn call_matches_textbook() {
        let g = black_scholes(OptionType::Call, S, K, T, R, V).unwrap();
        assert!((g.price - 4.76).abs() < 0.01, "call price {}", g.price);
    }

    #[test]
    fn put_matches_textbook() {
        let g = black_scholes(OptionType::Put, S, K, T, R, V).unwrap();
        assert!((g.price - 0.81).abs() < 0.01, "put price {}", g.price);
    }

    #[test]
    fn put_call_parity_holds() {
        let c = black_scholes(OptionType::Call, S, K, T, R, V).unwrap();
        let p = black_scholes(OptionType::Put, S, K, T, R, V).unwrap();
        let lhs = c.price - p.price;
        let rhs = S - K * (-R * T).exp();
        assert!((lhs - rhs).abs() < 1e-9, "parity violated: {lhs} vs {rhs}");
        // Parity holds across a second arbitrary point too.
        let c2 = black_scholes(OptionType::Call, 100.0, 95.0, 1.0, 0.05, 0.3).unwrap();
        let p2 = black_scholes(OptionType::Put, 100.0, 95.0, 1.0, 0.05, 0.3).unwrap();
        let lhs2 = c2.price - p2.price;
        let rhs2 = 100.0 - 95.0 * (-0.05_f64).exp();
        assert!((lhs2 - rhs2).abs() < 1e-9);
    }

    #[test]
    fn arbitrage_bounds_hold() {
        let c = black_scholes(OptionType::Call, S, K, T, R, V).unwrap();
        assert!(c.price >= (S - K * (-R * T).exp()).max(0.0) - 1e-9);
        assert!(c.price <= S + 1e-9);
        let p = black_scholes(OptionType::Put, S, K, T, R, V).unwrap();
        assert!(p.price >= (K * (-R * T).exp() - S).max(0.0) - 1e-9);
        assert!(p.price <= K * (-R * T).exp() + 1e-9);
    }

    #[test]
    fn delta_is_sane() {
        let c = black_scholes(OptionType::Call, S, K, T, R, V).unwrap();
        let p = black_scholes(OptionType::Put, S, K, T, R, V).unwrap();
        assert!((c.delta - 0.779).abs() < 0.005, "call delta {}", c.delta);
        assert!(
            (p.delta - (c.delta - 1.0)).abs() < 1e-9,
            "put/call delta relation"
        );
        assert!(c.gamma > 0.0 && c.vega > 0.0 && c.theta < 0.0);
    }

    #[test]
    fn expired_returns_intrinsic() {
        let g = black_scholes(OptionType::Call, 50.0, 45.0, 0.0, R, V).unwrap();
        assert_eq!(g.price, 5.0);
        assert_eq!(g.delta, 0.0);
        let g = black_scholes(OptionType::Put, 50.0, 55.0, -0.1, R, V).unwrap();
        assert_eq!(g.price, 5.0);
    }

    #[test]
    fn non_finite_fails_closed() {
        assert_eq!(
            black_scholes(OptionType::Call, f64::NAN, K, T, R, V).unwrap_err(),
            PricingError::NonFiniteInput("spot")
        );
        assert_eq!(
            black_scholes(OptionType::Call, S, K, T, R, -0.1).unwrap_err(),
            PricingError::NegativeVolatility(-0.1)
        );
    }

    #[test]
    fn norm_cdf_spot_checks() {
        assert!((norm_cdf(0.0) - 0.5).abs() < 1e-7);
        assert!((norm_cdf(1.96) - 0.975).abs() < 1e-4);
        assert!((norm_cdf(-1.96) - 0.025).abs() < 1e-4);
    }

    #[test]
    fn iv_rank_conventional() {
        let hist = vec![10.0, 20.0, 30.0, 40.0];
        assert_eq!(iv_rank(40.0, &hist), Some(100.0));
        assert_eq!(iv_rank(10.0, &hist), Some(0.0));
        assert_eq!(iv_rank(25.0, &hist), Some(50.0));
        assert_eq!(iv_rank(25.0, &[]), None);
        assert_eq!(iv_rank(25.0, &[20.0, 20.0]), None); // flat history: undefined
        assert_eq!(iv_rank(f64::NAN, &hist), None);
    }

    #[test]
    fn iv_percentile_counts_at_or_below() {
        let hist = vec![10.0, 20.0, 30.0, 40.0];
        assert_eq!(iv_percentile(40.0, &hist), Some(100.0));
        assert_eq!(iv_percentile(20.0, &hist), Some(50.0));
        assert_eq!(iv_percentile(5.0, &hist), Some(0.0));
        assert_eq!(iv_percentile(25.0, &[]), None);
        assert_eq!(iv_percentile(f64::NAN, &hist), None);
    }

    #[test]
    fn zero_vol_is_discounted_intrinsic_limit() {
        // S=100, K=100, r=5%, T=1: call -> 100 - 100*e^(-0.05).
        let g = black_scholes(OptionType::Call, 100.0, 100.0, 1.0, 0.05, 0.0).unwrap();
        let expected = 100.0 - 100.0 * (-0.05f64).exp();
        assert!((g.price - expected).abs() < 1e-12, "price {}", g.price);
        // Continuity: tiny vol approaches the same limit.
        let tiny = black_scholes(OptionType::Call, 100.0, 100.0, 1.0, 0.05, 1e-6).unwrap();
        assert!(
            (tiny.price - expected).abs() < 1e-4,
            "tiny-vol {}",
            tiny.price
        );
        // Put: K*e^(-rT) - S.
        let p = black_scholes(OptionType::Put, 100.0, 100.0, 1.0, 0.05, 0.0).unwrap();
        assert!((p.price - (100.0 * (-0.05f64).exp() - 100.0).max(0.0)).abs() < 1e-12);
    }

    #[test]
    fn determinism() {
        let a = black_scholes(OptionType::Call, S, K, T, R, V).unwrap();
        let b = black_scholes(OptionType::Call, S, K, T, R, V).unwrap();
        assert_eq!(a, b);
    }
}
