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
//! - **GARCH(1,1)** (v1): Gaussian maximum-likelihood fit with stationarity
//!   enforced (α + β < 1) through an unconstrained reparameterization, solved
//!   with a hand-rolled deterministic Nelder-Mead (no new dependencies).
//!   Needs ≥ 250 bars by default; multi-step forecasts mean-revert
//!   geometrically to the unconditional variance ω/(1−α−β).
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

// ---------------------------------------------------------------------------
// GARCH(1,1)
// ---------------------------------------------------------------------------

/// Logistic sigmoid: maps R onto (0, 1), used for constrained parameters.
#[inline]
fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Inverse sigmoid: maps (0, 1) back onto R.
#[inline]
fn logit(p: f64) -> f64 {
    debug_assert!(p > 0.0 && p < 1.0);
    (p / (1.0 - p)).ln()
}

/// Map an unconstrained R³ back to (ω, α, β) with ω > 0, α ≥ 0, β ≥ 0 and
/// α + β < 1 (strict): x0 = ln ω, x1 = logit(α+β), x2 = logit(α/(α+β)).
///
/// Stationarity is structural — the optimizer can never propose a
/// non-stationary model, so no penalty terms or rejections are needed
/// inside the likelihood loop.
fn unpack_garch(x: &[f64]) -> (f64, f64, f64) {
    let omega = x[0].exp();
    let persistence = sigmoid(x[1]);
    let alpha_share = sigmoid(x[2]);
    let alpha = persistence * alpha_share;
    let beta = persistence * (1.0 - alpha_share);
    (omega, alpha, beta)
}

/// Mean negative Gaussian log-likelihood of GARCH(1,1) at unconstrained
/// parameters `x`, over demeaned residuals `resid`.
///
/// The conditional variance recursion is seeded at `seed_var` (the sample
/// variance): the first residual's variance is the seed, so the likelihood
/// is conditional on the seed — the standard treatment. Returns +∞ if a
/// variance goes non-finite or non-positive, steering the optimizer away.
/// The ½·n·ln(2π) constant is dropped; the mean keeps the objective's scale
/// independent of the series length.
fn garch_objective(x: &[f64], resid: &[f64], seed_var: f64) -> f64 {
    let (omega, alpha, beta) = unpack_garch(x);
    let mut var = seed_var;
    let mut acc = 0.0;
    for &e in resid {
        if !var.is_finite() || var <= 0.0 {
            return f64::INFINITY;
        }
        acc += var.ln() + e * e / var;
        var = omega + alpha * e * e + beta * var;
    }
    0.5 * acc / resid.len() as f64
}

/// Deterministic Nelder-Mead simplex minimization over Rⁿ.
///
/// Classic coefficients (reflection 1, expansion 2, contraction 1/2, shrink
/// 1/2). Stops after `max_iter` iterations or when the spread of simplex
/// function values falls to `tol`. No RNG, no wall-clock reads, stable
/// ordering: identical inputs always produce the identical minimizer.
/// Hand-rolled to keep this crate dependency-free (SPEC-005 §5.1).
fn nelder_mead<F>(mut f: F, start: &[f64], step: &[f64], max_iter: usize, tol: f64) -> Vec<f64>
where
    F: FnMut(&[f64]) -> f64,
{
    let n = start.len();
    debug_assert!(n > 0 && step.len() == n);

    // Simplex: the start plus one vertex stepped along each coordinate.
    let mut simplex: Vec<Vec<f64>> = Vec::with_capacity(n + 1);
    simplex.push(start.to_vec());
    for i in 0..n {
        let mut v = start.to_vec();
        v[i] += step[i];
        simplex.push(v);
    }
    let mut values: Vec<f64> = simplex.iter().map(|v| f(v)).collect();

    // Per-iteration vertex order, best → worst. Rebuilt every iteration.
    let mut order: Vec<usize> = (0..=n).collect();
    for _ in 0..max_iter {
        order.sort_by(|&a, &b| {
            values[a]
                .partial_cmp(&values[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if (values[order[n]] - values[order[0]]).partial_cmp(&tol)
            != Some(std::cmp::Ordering::Greater)
        {
            break; // converged (also breaks if the spread is NaN)
        }
        // Centroid of the n best vertices.
        let mut centroid = vec![0.0; n];
        for &idx in order[..n].iter() {
            for j in 0..n {
                centroid[j] += simplex[idx][j];
            }
        }
        for c in centroid.iter_mut() {
            *c /= n as f64;
        }
        let worst = order[n];
        let f_worst = values[worst];
        let f_second = values[order[n - 1]];
        let f_best = values[order[0]];

        let reflected: Vec<f64> = centroid
            .iter()
            .zip(simplex[worst].iter())
            .map(|(c, w)| c + (c - w))
            .collect();
        let f_reflected = f(&reflected);

        if f_reflected < f_best {
            let expanded: Vec<f64> = centroid
                .iter()
                .zip(reflected.iter())
                .map(|(c, r)| c + 2.0 * (r - c))
                .collect();
            let f_expanded = f(&expanded);
            if f_expanded < f_reflected {
                simplex[worst] = expanded;
                values[worst] = f_expanded;
            } else {
                simplex[worst] = reflected;
                values[worst] = f_reflected;
            }
        } else if f_reflected < f_second {
            simplex[worst] = reflected;
            values[worst] = f_reflected;
        } else {
            // Contract: outside if reflection beat the worst, else inside.
            let contracted: Vec<f64> = if f_reflected < f_worst {
                centroid
                    .iter()
                    .zip(reflected.iter())
                    .map(|(c, r)| c + 0.5 * (r - c))
                    .collect()
            } else {
                centroid
                    .iter()
                    .zip(simplex[worst].iter())
                    .map(|(c, w)| c + 0.5 * (w - c))
                    .collect()
            };
            let f_contracted = f(&contracted);
            if f_contracted < f_worst {
                simplex[worst] = contracted;
                values[worst] = f_contracted;
            } else {
                // Shrink every vertex toward the best.
                let best_pt = simplex[order[0]].clone();
                for &idx in order.iter().take(n + 1).skip(1) {
                    for j in 0..n {
                        simplex[idx][j] = best_pt[j] + 0.5 * (simplex[idx][j] - best_pt[j]);
                    }
                    values[idx] = f(&simplex[idx]);
                }
            }
        }
    }
    order.sort_by(|&a, &b| {
        values[a]
            .partial_cmp(&values[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    simplex[order[0]].clone()
}

/// Full GARCH(1,1) fit: the [`VolatilityFit`] plus the fitted parameters and
/// the recursion tail needed for multi-step-ahead forecasts.
#[derive(Debug, Clone)]
pub struct GarchFit {
    /// The one-step-ahead fit (same shape as every other estimator).
    pub fit: VolatilityFit,
    /// Fitted ω, α, β.
    pub omega: f64,
    pub alpha: f64,
    pub beta: f64,
    /// Final conditional variance σ²_n — the one-step-ahead forecast.
    pub last_variance: f64,
}

impl GarchFit {
    /// k-step-ahead conditional variance forecasts.
    ///
    /// The first value equals `fit.variance`; each later step applies
    /// σ²_{t+1} = ω + (α+β)·σ²_t, so the path mean-reverts geometrically to
    /// the unconditional variance ω/(1−α−β).
    pub fn forecast_variances(&self, steps: usize) -> Vec<f64> {
        let mut out = Vec::with_capacity(steps);
        let persistence = self.alpha + self.beta;
        let mut v = self.last_variance;
        for _ in 0..steps {
            out.push(v);
            v = self.omega + persistence * v;
        }
        out
    }

    /// Unconditional (long-run) variance ω/(1−α−β).
    pub fn unconditional_variance(&self) -> f64 {
        self.omega / (1.0 - self.alpha - self.beta)
    }
}

/// GARCH(1,1) volatility estimator (SPEC-005 v1).
///
/// Model: σ²_t = ω + α·ε²_{t-1} + β·σ²_{t-1}, where ε_t = r_t − μ are the
/// demeaned log returns. Fit by Gaussian maximum likelihood; ω > 0, α ≥ 0,
/// β ≥ 0, α + β < 1 are enforced structurally by [`unpack_garch`], and the
/// likelihood is maximized by a deterministic Nelder-Mead from five fixed
/// starting points spanning the plausible persistence range (best likelihood
/// wins — no RNG anywhere, so fits are bit-identical).
///
/// The variance recursion is seeded at the sample variance; its final value
/// is the one-step-ahead forecast. The default minimum is 250 returns (≈ 1
/// trading year) — MLE on shorter histories is numerology, so shorter input
/// fails closed. A `boundary_solution` diagnostic of 1.0 means the fit ran
/// to the α+β → 1 edge (near-integrated variance); treat the forecast with
/// suspicion and prefer EWMA.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Garch11 {
    /// Minimum returns required (default 250).
    pub min_returns: usize,
}

impl Garch11 {
    /// Standard configuration: 250-return minimum.
    pub fn standard() -> Self {
        Self { min_returns: 250 }
    }

    /// Fit the model and return the full [`GarchFit`], including parameters
    /// and the recursion tail for horizon forecasts.
    pub fn fit_detailed(
        &self,
        returns: &[f64],
        periods_per_year: f64,
    ) -> Result<GarchFit, FitError> {
        if self.min_returns < 50 {
            return Err(FitError::InvalidParameter(format!(
                "min_returns must be >= 50 for GARCH(1,1), got {}",
                self.min_returns
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

        let n = returns.len();
        // Demean: the Gaussian likelihood models ε_t = r_t − μ.
        let mean = returns.iter().sum::<f64>() / n as f64;
        let resid: Vec<f64> = returns.iter().map(|r| r - mean).collect();
        let seed_var = resid.iter().map(|e| e * e).sum::<f64>() / (n - 1) as f64;

        if seed_var == 0.0 {
            // Constant returns: the honest forecast is zero variance. The
            // likelihood is degenerate here (infinite at ω → 0), so report
            // the limit directly instead of optimizing after a ghost.
            return Ok(zero_variance_fit(n, mean));
        }

        // Five fixed starts across the plausible persistence range. Each ω
        // start puts the implied unconditional variance at the seed, so the
        // optimizer begins near a sane scale however persistent the data is.
        let starts: [[f64; 3]; 5] = [
            (0.80, 0.20),
            (0.90, 0.15),
            (0.95, 0.05),
            (0.99, 0.10),
            (0.50, 0.30),
        ]
        .map(|(p, q)| [(seed_var * (1.0 - p)).ln(), logit(p), logit(q)]);
        let step = [0.5, 0.75, 0.75];
        let mut best_x = starts[0].to_vec();
        let mut best_f = f64::INFINITY;
        for s in &starts {
            let x = nelder_mead(
                |v| garch_objective(v, &resid, seed_var),
                s,
                &step,
                1000,
                1e-12,
            );
            let f = garch_objective(&x, &resid, seed_var);
            if f < best_f {
                best_f = f;
                best_x = x;
            }
        }
        if !best_f.is_finite() {
            return Err(FitError::NoConvergence(
                "GARCH(1,1) optimizer found no finite likelihood".to_string(),
            ));
        }
        let (omega, alpha, beta) = unpack_garch(&best_x);

        // Final pass: one-step-ahead variance and the maximized log-likelihood.
        let mut var = seed_var;
        let mut loglik = 0.0;
        for &e in &resid {
            loglik += -0.5 * (std::f64::consts::TAU.ln() + var.ln() + e * e / var);
            var = omega + alpha * e * e + beta * var;
        }
        let variance = var; // σ²_n: the one-step-ahead forecast
        let persistence = alpha + beta;
        let unconditional = omega / (1.0 - persistence);
        if !variance.is_finite()
            || variance < 0.0
            || !loglik.is_finite()
            || !unconditional.is_finite()
        {
            return Err(FitError::NoConvergence(
                "GARCH(1,1) fit produced a non-finite forecast or likelihood".to_string(),
            ));
        }

        let sigma = variance.sqrt();
        let fit = VolatilityFit {
            estimator: self.name().to_string(),
            variance,
            sigma,
            annualized_sigma: sigma * periods_per_year.sqrt(),
            persistence,
            half_life_periods: 0.5f64.ln() / persistence.ln(),
            n_returns: n,
            // Sorted by key for deterministic output.
            diagnostics: vec![
                ("alpha".to_string(), alpha),
                ("beta".to_string(), beta),
                (
                    "boundary_solution".to_string(),
                    if persistence > 0.999 { 1.0 } else { 0.0 },
                ),
                ("log_likelihood".to_string(), loglik),
                ("mean".to_string(), mean),
                ("omega".to_string(), omega),
                ("seed_variance".to_string(), seed_var),
                ("unconditional_variance".to_string(), unconditional),
            ],
        };
        Ok(GarchFit {
            fit,
            omega,
            alpha,
            beta,
            last_variance: variance,
        })
    }
}

/// Degenerate fit for constant returns: zero variance, honestly reported.
/// The Gaussian log-likelihood is infinite at the ω → 0 limit, so it is
/// omitted from diagnostics rather than fabricated.
fn zero_variance_fit(n: usize, mean: f64) -> GarchFit {
    let persistence = 0.9;
    let fit = VolatilityFit {
        estimator: "garch11".to_string(),
        variance: 0.0,
        sigma: 0.0,
        annualized_sigma: 0.0,
        persistence,
        half_life_periods: 0.5f64.ln() / persistence.ln(),
        n_returns: n,
        diagnostics: vec![
            ("alpha".to_string(), 0.0),
            ("beta".to_string(), persistence),
            ("boundary_solution".to_string(), 0.0),
            ("mean".to_string(), mean),
            ("omega".to_string(), 0.0),
            ("seed_variance".to_string(), 0.0),
            ("unconditional_variance".to_string(), 0.0),
        ],
    };
    GarchFit {
        fit,
        omega: 0.0,
        alpha: 0.0,
        beta: persistence,
        last_variance: 0.0,
    }
}

impl VolatilityEstimator for Garch11 {
    fn name(&self) -> &'static str {
        "garch11"
    }

    fn min_returns(&self) -> usize {
        self.min_returns
    }

    fn fit(&self, returns: &[f64], periods_per_year: f64) -> Result<VolatilityFit, FitError> {
        Ok(self.fit_detailed(returns, periods_per_year)?.fit)
    }
}

/// Resolve an estimator by name.
///
/// `"garch11"` is the SPEC-005 v1 Gaussian-MLE estimator (≥ 250 returns by
/// default). `lambda` is an EWMA-only option and is rejected for garch11
/// rather than silently ignored.
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
        "garch11" => {
            if lambda.is_some() {
                return Err(FitError::InvalidParameter(
                    "--lambda is an EWMA option; garch11 takes no lambda".to_string(),
                ));
            }
            Ok(Box::new(Garch11 {
                min_returns: min_returns.unwrap_or(250),
            }))
        }
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

    /// Deterministic PRNG (xorshift64*): reproducible synthetic data with no
    /// external RNG crate.
    struct XorShift64(u64);

    impl XorShift64 {
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        /// Double in [0, 1) with 53 bits of precision.
        fn next_unit(&mut self) -> f64 {
            const DENOM: f64 = (1u64 << 53) as f64;
            ((self.next_u64() >> 11) as f64) / DENOM
        }

        /// Standard normal via Box-Muller.
        fn next_normal(&mut self) -> f64 {
            let u1 = self.next_unit().max(f64::MIN_POSITIVE);
            let u2 = self.next_unit();
            (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
        }
    }

    /// Synthetic GARCH(1,1) returns with known parameters (zero mean).
    fn synthetic_garch(n: usize, omega: f64, alpha: f64, beta: f64, seed: u64) -> Vec<f64> {
        let mut rng = XorShift64(seed);
        let mut var = omega / (1.0 - alpha - beta);
        (0..n)
            .map(|_| {
                let e = rng.next_normal() * var.sqrt();
                var = omega + alpha * e * e + beta * var;
                e
            })
            .collect()
    }

    #[test]
    fn garch11_recovers_known_parameters() {
        let (omega, alpha, beta) = (2e-6, 0.07, 0.90);
        let returns = synthetic_garch(3000, omega, alpha, beta, 0xC10C);
        let gfit = Garch11::standard().fit_detailed(&returns, 252.0).unwrap();
        assert!(
            (gfit.alpha - alpha).abs() < 0.03,
            "alpha {} not near {alpha}",
            gfit.alpha
        );
        assert!(
            (gfit.beta - beta).abs() < 0.03,
            "beta {} not near {beta}",
            gfit.beta
        );
        assert!(
            (gfit.omega.ln() - omega.ln()).abs() < 1.0,
            "omega {} not near {omega}",
            gfit.omega
        );
        assert!(gfit.fit.persistence < 1.0, "persistence must stay < 1");
        assert_eq!(gfit.fit.estimator, "garch11");
        assert_eq!(gfit.fit.n_returns, 3000);
        // Annualization follows the same convention as EWMA.
        assert!((gfit.fit.annualized_sigma - gfit.fit.sigma * 252f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn garch11_stays_stationary_on_real_shaped_data() {
        let fit = Garch11::standard()
            .fit(&fixture_returns(500), 252.0)
            .unwrap();
        assert!(fit.persistence < 1.0 && fit.persistence >= 0.0);
        assert!(fit.variance > 0.0 && fit.variance.is_finite());
        assert!(fit.diagnostics.iter().all(|(_, v)| v.is_finite()));
        // Diagnostics are sorted by key for deterministic output.
        let keys: Vec<&str> = fit.diagnostics.iter().map(|(k, _)| k.as_str()).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn garch11_forecast_converges_to_unconditional_variance() {
        let gfit = Garch11::standard()
            .fit_detailed(&synthetic_garch(2000, 2e-6, 0.10, 0.85, 7), 252.0)
            .unwrap();
        let u = gfit.unconditional_variance();
        let p = gfit.alpha + gfit.beta;
        // Exact geometric mean-reversion: v_k = u + p^k (v_0 − u).
        let path = gfit.forecast_variances(6);
        assert_eq!(path.len(), 6);
        assert_eq!(path[0], gfit.last_variance);
        let expected_5 = u + p.powi(5) * (path[0] - u);
        assert!(
            (path[5] - expected_5).abs() <= 1e-9 * u.max(1e-12),
            "path[5]={} expected {expected_5}",
            path[5]
        );
        // …and the long horizon keeps approaching the unconditional variance.
        let long = gfit.forecast_variances(1000);
        let expected_last = u + p.powi(999) * (path[0] - u);
        assert!(
            (long[999] - expected_last).abs() <= 1e-9 * u.max(1e-12),
            "long[999]={} expected {expected_last}",
            long[999]
        );
        assert!(
            (long[999] - u).abs() <= (path[0] - u).abs(),
            "forecast should approach the unconditional variance"
        );
    }

    #[test]
    fn garch11_rejects_short_series() {
        let est = Garch11::standard();
        let err = est.fit(&fixture_returns(249), 252.0).unwrap_err();
        assert_eq!(err, FitError::InsufficientData { min: 250, got: 249 });
    }

    #[test]
    fn garch11_rejects_tiny_minimum() {
        let est = Garch11 { min_returns: 49 };
        assert!(matches!(
            est.fit(&fixture_returns(100), 252.0).unwrap_err(),
            FitError::InvalidParameter(_)
        ));
    }

    #[test]
    fn garch11_rejects_non_finite_input() {
        let est = Garch11 { min_returns: 50 };
        let mut returns = fixture_returns(60);
        returns[4] = f64::NAN;
        assert_eq!(
            est.fit(&returns, 252.0).unwrap_err(),
            FitError::NonFiniteInput(4)
        );
    }

    #[test]
    fn garch11_rejects_lambda() {
        let err = estimator_by_name("garch11", Some(0.94), None).unwrap_err();
        assert!(matches!(err, FitError::InvalidParameter(_)));
    }

    #[test]
    fn garch11_resolves_by_name() {
        let est = estimator_by_name("garch11", None, None).unwrap();
        assert_eq!(est.name(), "garch11");
        assert_eq!(est.min_returns(), 250);
    }

    #[test]
    fn garch11_handles_constant_returns() {
        // Degenerate but valid input: the honest forecast is zero variance.
        let est = Garch11 { min_returns: 50 };
        let fit = est.fit(&vec![0.0; 300], 252.0).unwrap();
        assert_eq!(fit.variance, 0.0);
        assert_eq!(fit.annualized_sigma, 0.0);
        assert!(fit.diagnostics.iter().all(|(_, v)| v.is_finite()));
        // Float dust (a non-representable constant like 0.001 leaves ~1e-19
        // residuals after demeaning) still yields a finite, economically-zero
        // forecast rather than an error.
        let dusty = est.fit(&vec![0.001; 300], 252.0).unwrap();
        assert!(dusty.variance.is_finite() && dusty.variance < 1e-30);
    }

    #[test]
    fn garch11_is_deterministic() {
        let returns = fixture_returns(500);
        let est = Garch11::standard();
        let a = est.fit(&returns, 252.0).unwrap();
        let b = est.fit(&returns, 252.0).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn unknown_estimator_fails_closed() {
        let err = estimator_by_name("gjr-garch", None, None).unwrap_err();
        assert_eq!(err, FitError::NotImplemented("gjr-garch".to_string()));
    }
}
