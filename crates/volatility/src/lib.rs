//! Volatility forecasting for Thales.
//!
//! Estimates the one-step-ahead conditional variance of a return series and
//! reports it as per-period and annualized volatility. A volatility forecast
//! is **evidence, not a trade signal**: it feeds position sizing and risk
//! checks, it never clears the Jev gate on its own.
//!
//! # Estimators
//!
//! - **EWMA** (v0, this crate): the RiskMetrics exponentially-weighted moving
//!   variance. Deterministic, closed-form, no optimization. The default
//!   estimator and the right one for short histories.
//! - **GARCH(1,1)** (v1, planned): Gaussian maximum-likelihood fit with
//!   stationarity enforced (α + β < 1). Needs ≥ 250 bars and a deterministic
//!   optimizer; not implemented yet — requesting it fails closed.
//!
//! All estimators are pure functions of the input returns: no randomness, no
//! I/O, no hidden state. Fitting the same returns twice yields bit-identical
//! fits.

use thiserror::Error;

pub mod ohlc;
pub use ohlc::{Ohlc, combine_forecasts, garman_klass, parkinson, rogers_satchell, yang_zhang};

/// Errors from volatility estimation. Every failure mode is typed so callers
/// (and the CLI) fail closed instead of silently returning a nonsense number.
#[derive(Debug, Error, PartialEq)]
pub enum FitError {
    /// Fewer usable returns than the estimator's minimum.
    #[error("insufficient data: need at least {min} returns, got {got}")]
    InsufficientData { min: usize, got: usize },
    /// A return (or seed price) was NaN, infinite, or non-positive where a
    /// positive price was required.
    #[error("non-finite or invalid input at position {0}")]
    NonFiniteInput(usize),
    /// The optimizer did not converge within its deterministic budget.
    #[error("optimizer failed to converge: {0}")]
    NoConvergence(String),
    /// An estimator parameter was outside its valid range.
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),
    /// The requested estimator is specified but not implemented yet.
    #[error("estimator '{0}' is not implemented yet")]
    NotImplemented(String),
}

/// The result of fitting a volatility model to a return series.
#[derive(Debug, Clone, PartialEq)]
pub struct VolatilityFit {
    /// Name of the estimator that produced this fit (`"ewma"`, `"garch11"`).
    pub estimator: String,
    /// One-step-ahead conditional variance, in per-period (e.g. daily) units.
    pub variance: f64,
    /// One-step-ahead conditional volatility (σ = √variance), per period.
    pub sigma: f64,
    /// Annualized volatility: `sigma * sqrt(periods_per_year)`.
    pub annualized_sigma: f64,
    /// Persistence of the variance process (EWMA: λ; GARCH: α + β).
    pub persistence: f64,
    /// Shock half-life in periods: `ln(0.5) / ln(persistence)`.
    pub half_life_periods: f64,
    /// Number of returns the fit consumed.
    pub n_returns: usize,
    /// Estimator-specific diagnostics (seed variance, log-likelihood, ...).
    /// Sorted by key for deterministic output.
    pub diagnostics: Vec<(String, f64)>,
}

/// A volatility estimator: a named, deterministic fit over log returns.
pub trait VolatilityEstimator: std::fmt::Debug {
    /// Human-readable name used in output and logs.
    fn name(&self) -> &'static str;

    /// Minimum number of returns required. Fewer → [`FitError::InsufficientData`].
    fn min_returns(&self) -> usize;

    /// Fit the estimator to `returns` (log returns, oldest first) and return
    /// the one-step-ahead forecast. Pure and deterministic.
    ///
    /// `periods_per_year` annualizes σ (252 for daily equities, 365 for crypto).
    fn fit(&self, returns: &[f64], periods_per_year: f64) -> Result<VolatilityFit, FitError>;
}

/// Compute log returns `ln(p_t / p_{t-1})` from a price series.
///
/// Fails closed on non-finite or non-positive prices — a zero or negative
/// "price" is corrupt data, not something to smooth over.
pub fn log_returns_from_prices(prices: &[f64]) -> Result<Vec<f64>, FitError> {
    if prices.len() < 2 {
        return Err(FitError::InsufficientData {
            min: 2,
            got: prices.len(),
        });
    }
    let mut returns = Vec::with_capacity(prices.len() - 1);
    for (i, window) in prices.windows(2).enumerate() {
        let (prev, curr) = (window[0], window[1]);
        if !prev.is_finite() || !curr.is_finite() || prev <= 0.0 || curr <= 0.0 {
            return Err(FitError::NonFiniteInput(i));
        }
        returns.push((curr / prev).ln());
    }
    Ok(returns)
}

/// EWMA (RiskMetrics) volatility estimator.
///
/// Recursion: `σ²_t = λ·σ²_{t-1} + (1-λ)·r_t²`, seeded with the sample
/// variance of the input returns. The final `σ²` is the one-step-ahead
/// forecast. Deterministic and closed-form — no optimizer involved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ewma {
    /// Decay factor, in (0, 1). RiskMetrics standard for daily data: 0.94.
    pub lambda: f64,
    /// Minimum returns required (default 60 ≈ 3 trading months).
    pub min_returns: usize,
}

impl Ewma {
    /// Standard RiskMetrics daily configuration: λ = 0.94, min 60 returns.
    pub fn riskmetrics_daily() -> Self {
        Self {
            lambda: 0.94,
            min_returns: 60,
        }
    }
}

impl VolatilityEstimator for Ewma {
    fn name(&self) -> &'static str {
        "ewma"
    }

    fn min_returns(&self) -> usize {
        self.min_returns
    }

    fn fit(&self, returns: &[f64], periods_per_year: f64) -> Result<VolatilityFit, FitError> {
        if !(self.lambda > 0.0 && self.lambda < 1.0) {
            return Err(FitError::InvalidParameter(format!(
                "lambda must be in (0, 1), got {}",
                self.lambda
            )));
        }
        if !periods_per_year.is_finite() || periods_per_year <= 0.0 {
            return Err(FitError::InvalidParameter(format!(
                "periods_per_year must be positive, got {periods_per_year}"
            )));
        }
        if returns.len() < self.min_returns {
            return Err(FitError::InsufficientData {
                min: self.min_returns,
                got: returns.len(),
            });
        }
        for (i, r) in returns.iter().enumerate() {
            if !r.is_finite() {
                return Err(FitError::NonFiniteInput(i));
            }
        }

        // Seed with the sample variance (Bessel-corrected); the recursion
        // below then applies exponential weights to every return.
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let seed =
            returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;

        let mut variance = seed;
        let update = 1.0 - self.lambda;
        for r in returns {
            variance = self.lambda * variance + update * r * r;
        }
        if !variance.is_finite() || variance < 0.0 {
            return Err(FitError::NoConvergence(
                "EWMA recursion produced a non-finite variance".to_string(),
            ));
        }

        let sigma = variance.sqrt();
        let half_life_periods = 0.5f64.ln() / self.lambda.ln();
        Ok(VolatilityFit {
            estimator: self.name().to_string(),
            variance,
            sigma,
            annualized_sigma: sigma * periods_per_year.sqrt(),
            persistence: self.lambda,
            half_life_periods,
            n_returns: returns.len(),
            diagnostics: vec![("seed_variance".to_string(), seed)],
        })
    }
}

/// Resolve an estimator by name. `"garch11"` is specified (SPEC-005 v1) but
/// not implemented — it fails closed rather than returning a wrong model.
pub fn estimator_by_name(
    name: &str,
    lambda: Option<f64>,
    min_returns: Option<usize>,
) -> Result<Box<dyn VolatilityEstimator>, FitError> {
    match name {
        "ewma" => Ok(Box::new(Ewma {
            lambda: lambda.unwrap_or(0.94),
            min_returns: min_returns.unwrap_or(60),
        })),
        "garch11" => Err(FitError::NotImplemented("garch11".to_string())),
        other => Err(FitError::NotImplemented(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random returns: sine-based, no RNG needed.
    fn fixture_returns(n: usize) -> Vec<f64> {
        (0..n)
            .map(|i| 0.01 * ((i as f64 * 0.7).sin() + 0.5 * (i as f64 * 0.23).cos()))
            .collect()
    }

    #[test]
    fn ewma_matches_hand_computation() {
        // Hand-computable case: constant returns r => variance converges to r^2
        // regardless of seed, so the forecast must equal r^2 up to the seed's
        // exponentially-decayed contribution.
        let lambda = 0.94;
        let r = 0.02;
        let returns = vec![r; 500];
        let est = Ewma {
            lambda,
            min_returns: 60,
        };
        let fit = est.fit(&returns, 252.0).unwrap();
        // After 500 updates the seed contribution is lambda^500 ~ 4e-14.
        let expected_variance = r * r;
        assert!(
            (fit.variance - expected_variance).abs() < 1e-9,
            "variance {} != {}",
            fit.variance,
            expected_variance
        );
        assert!((fit.sigma - r).abs() < 1e-9);
        assert!((fit.annualized_sigma - r * 252f64.sqrt()).abs() < 1e-9);
        assert_eq!(fit.persistence, lambda);
        assert!((fit.half_life_periods - 0.5f64.ln() / lambda.ln()).abs() < 1e-12);
        assert_eq!(fit.n_returns, 500);
    }

    #[test]
    fn ewma_seed_is_sample_variance() {
        let returns = fixture_returns(100);
        let est = Ewma::riskmetrics_daily();
        let fit = est.fit(&returns, 252.0).unwrap();
        let mean = returns.iter().sum::<f64>() / 100.0;
        let expected_seed = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / 99.0;
        assert_eq!(
            fit.diagnostics,
            vec![("seed_variance".to_string(), expected_seed)]
        );
    }

    #[test]
    fn ewma_rejects_short_series() {
        let est = Ewma::riskmetrics_daily();
        let err = est.fit(&fixture_returns(59), 252.0).unwrap_err();
        assert_eq!(err, FitError::InsufficientData { min: 60, got: 59 });
    }

    #[test]
    fn ewma_rejects_non_finite_input() {
        let est = Ewma {
            lambda: 0.94,
            min_returns: 3,
        };
        let mut returns = fixture_returns(10);
        returns[4] = f64::NAN;
        assert_eq!(
            est.fit(&returns, 252.0).unwrap_err(),
            FitError::NonFiniteInput(4)
        );
    }

    #[test]
    fn ewma_rejects_bad_lambda() {
        for bad in [0.0, 1.0, 1.5, f64::NAN] {
            let est = Ewma {
                lambda: bad,
                min_returns: 3,
            };
            assert!(
                matches!(
                    est.fit(&fixture_returns(10), 252.0).unwrap_err(),
                    FitError::InvalidParameter(_)
                ),
                "lambda={bad} should fail"
            );
        }
    }

    #[test]
    fn ewma_is_deterministic() {
        let returns = fixture_returns(200);
        let est = Ewma::riskmetrics_daily();
        let a = est.fit(&returns, 252.0).unwrap();
        let b = est.fit(&returns, 252.0).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn log_returns_are_correct() {
        let prices = vec![100.0, 110.0, 99.0];
        let r = log_returns_from_prices(&prices).unwrap();
        assert_eq!(r.len(), 2);
        assert!((r[0] - (1.1f64).ln()).abs() < 1e-15);
        assert!((r[1] - (0.9f64).ln()).abs() < 1e-15);
    }

    #[test]
    fn log_returns_reject_bad_prices() {
        assert!(log_returns_from_prices(&[100.0]).is_err());
        assert!(matches!(
            log_returns_from_prices(&[100.0, 0.0]).unwrap_err(),
            FitError::NonFiniteInput(0)
        ));
        assert!(matches!(
            log_returns_from_prices(&[100.0, f64::INFINITY]).unwrap_err(),
            FitError::NonFiniteInput(0)
        ));
    }

    #[test]
    fn garch11_fails_closed() {
        let err = estimator_by_name("garch11", None, None).unwrap_err();
        assert_eq!(err, FitError::NotImplemented("garch11".to_string()));
    }

    #[test]
    fn unknown_estimator_fails_closed() {
        let err = estimator_by_name("gjr-garch", None, None).unwrap_err();
        assert_eq!(err, FitError::NotImplemented("gjr-garch".to_string()));
    }
}
