//! Forecast-vs-implied edge (Sinclair framework, Part 2).
//!
//! The core claim: an option is cheap when your volatility forecast exceeds
//! its implied volatility, rich when implied exceeds your forecast. These
//! are small, honest helpers around that identity — arithmetic, not magic.
//!
//! - [`vol_edge`]: forecast − implied. Positive = cheap (buy vol).
//! - [`variance_risk_premium`]: implied − realized. Positive = sellers got
//!   paid (the structural reason short-vol works on equity indexes).
//! - [`edge_zscore`]: edge in units of forecast uncertainty — the sizing
//!   input. Bigger |z|, bigger position; never levered to ruin.
//! - [`gamma_theta_pnl_per_day`]: expected daily P&L of a delta-hedged
//!   position from the gamma/theta trade: `½·Γ·S²·(σ_real² − σ_impl²)/365`.
//!   Positive for long gamma when realized exceeds implied. Hedging costs
//!   and spreads come out of this — it is gross, not net.

/// Signed edge: `forecast_vol - implied_vol` (both annualized decimals).
///
/// Positive means the option is cheap relative to your forecast.
pub fn vol_edge(forecast_vol: f64, implied_vol: f64) -> f64 {
    forecast_vol - implied_vol
}

/// Variance risk premium: `implied_vol - realized_vol`.
///
/// Persistently positive on equity indexes — the premium sellers collect
/// for bearing gap/crash risk, and the reason Thales demands a wider edge
/// before *buying* index vol.
pub fn variance_risk_premium(implied_vol: f64, realized_vol: f64) -> f64 {
    implied_vol - realized_vol
}

/// Edge in units of forecast uncertainty: `(forecast − implied) / std`.
///
/// Returns `None` when `forecast_std` is not positive finite (fail closed —
/// never size off an undefined z-score).
pub fn edge_zscore(forecast_vol: f64, forecast_std: f64, implied_vol: f64) -> Option<f64> {
    if !forecast_std.is_finite() || forecast_std <= 0.0 {
        return None;
    }
    if !forecast_vol.is_finite() || !implied_vol.is_finite() {
        return None;
    }
    Some((forecast_vol - implied_vol) / forecast_std)
}

/// Expected daily delta-hedged P&L from gamma vs theta.
///
/// A delta-hedged long-gamma position earns `½·Γ·S²·realized_var·dt` from
/// gamma scalping and pays `θ·dt` in decay; under Black-Scholes theta is
/// priced at implied vol, so the net is proportional to
/// `(σ_real² − σ_impl²)`. Gross of hedging transaction costs.
pub fn gamma_theta_pnl_per_day(gamma: f64, spot: f64, realized_vol: f64, implied_vol: f64) -> f64 {
    0.5 * gamma * spot * spot * (realized_vol * realized_vol - implied_vol * implied_vol) / 365.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_signs() {
        assert!(vol_edge(0.30, 0.20) > 0.0); // cheap: buy vol
        assert!(vol_edge(0.15, 0.20) < 0.0); // rich: sell vol
        assert!(variance_risk_premium(0.20, 0.15) > 0.0); // sellers paid
    }

    #[test]
    fn zscore_math_and_fail_closed() {
        let z = edge_zscore(0.30, 0.05, 0.20).unwrap();
        assert!((z - 2.0).abs() < 1e-12, "z {z}");
        assert_eq!(edge_zscore(0.30, 0.0, 0.20), None);
        assert_eq!(edge_zscore(0.30, -0.05, 0.20), None);
        assert_eq!(edge_zscore(f64::NAN, 0.05, 0.20), None);
    }

    #[test]
    fn gamma_pnl_zero_when_realized_equals_implied() {
        let pnl = gamma_theta_pnl_per_day(0.05, 100.0, 0.20, 0.20);
        assert!(pnl.abs() < 1e-12, "pnl {pnl}");
    }

    #[test]
    fn long_gamma_profits_when_realized_exceeds_implied() {
        let pnl = gamma_theta_pnl_per_day(0.05, 100.0, 0.30, 0.20);
        // 0.5 * 0.05 * 10000 * (0.09 - 0.04) / 365 = 0.5*0.05*10000*0.05/365
        let expected = 0.5 * 0.05 * 100.0 * 100.0 * 0.05 / 365.0;
        assert!((pnl - expected).abs() < 1e-9, "pnl {pnl}");
        assert!(pnl > 0.0);
    }
}
