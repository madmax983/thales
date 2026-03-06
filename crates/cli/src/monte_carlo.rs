//! # Monte Carlo Simulation
//!
//! This module provides tools for running Monte Carlo simulations on backtest results.
//!
//! By resampling the historical trades from a [`BacktestResult`], it generates many possible
//! future equity curves. This helps in understanding the variance, potential drawdowns,
//! and the risk of ruin for a given trading strategy.

use crate::backtest::BacktestResult;
use anyhow::Result;
use rand::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration for the Monte Carlo simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloConfig {
    /// The number of simulation paths to generate.
    pub iterations: usize,
    /// Number of trades to simulate per path. Defaults to the number of trades in the backtest.
    pub horizon: Option<usize>,
    /// Starting capital for the simulation. Defaults to the backtest initial capital.
    pub initial_capital: Option<f64>,
}

/// A report containing the aggregated statistics from all simulation paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloReport {
    /// The number of simulation paths to generate.
    pub iterations: usize,
    pub horizon: usize,
    /// The total return percentage of the original backtest.
    pub baseline_return_pct: f64,
    /// The median return percentage across all simulated paths.
    pub median_return_pct: f64,
    /// The 95th percentile return percentage (Best Case).
    pub best_case_return_pct: f64,
    /// The 5th percentile return percentage (Worst Case).
    pub worst_case_return_pct: f64,
    /// The percentage of paths where the equity dropped to or below zero.
    pub risk_of_ruin_pct: f64,
    /// The median maximum drawdown percentage across all simulated paths.
    pub max_drawdown_median_pct: f64,
    /// Value at Risk (95% confidence). Represents the percentage loss of capital at the 5th percentile.
    pub var_95_pct: f64,
}

/// Runs a Monte Carlo simulation on a set of backtest trades.
///
/// Randomly samples (with replacement) from the backtest's trades to generate multiple simulated
/// equity curves, and then calculates aggregate statistics across those paths.
///
/// # Arguments
/// * `backtest` - The original backtest result containing the trades to sample.
/// * `config` - The configuration dictating the number of iterations and horizon.
///
/// # Errors
/// Returns an error if the backtest contains no trades to simulate.
///
/// # Examples
/// ```
/// use thales_cli::backtest::{BacktestResult, BacktestMetrics, BacktestTrade};
/// use thales_cli::monte_carlo::{run_simulation, MonteCarloConfig};
///
/// let trades = vec![
///     BacktestTrade { id: "1".to_string(), entry_time: 0, exit_time: 1, side: "long".to_string(), qty: 1.0, entry_price: 100.0, exit_price: 110.0, pnl: 100.0, pnl_pct: 0.1, exit_reason: "tp".to_string() },
///     BacktestTrade { id: "2".to_string(), entry_time: 2, exit_time: 3, side: "long".to_string(), qty: 1.0, entry_price: 110.0, exit_price: 105.0, pnl: -50.0, pnl_pct: -0.05, exit_reason: "sl".to_string() },
///     BacktestTrade { id: "3".to_string(), entry_time: 4, exit_time: 5, side: "long".to_string(), qty: 1.0, entry_price: 105.0, exit_price: 125.0, pnl: 200.0, pnl_pct: 0.2, exit_reason: "tp".to_string() },
/// ];
/// let backtest = BacktestResult {
///     strategy: "Test".to_string(),
///     symbol: "TEST".to_string(),
///     timeframe: "1d".to_string(),
///     initial_capital: 1000.0,
///     final_equity: 1250.0,
///     metrics: BacktestMetrics { total_return_pct: 25.0, cagr: 0.0, max_drawdown_pct: 5.0, win_rate: 0.66, profit_factor: 6.0, total_trades: 3, winning_trades: 2, losing_trades: 1 },
///     trades,
///     equity_curve: vec![],
/// };
/// let config = MonteCarloConfig {
///     iterations: 1000,
///     horizon: Some(10),
///     initial_capital: None,
/// };
/// let report = run_simulation(&backtest, config).unwrap();
/// assert!(report.iterations == 1000);
/// ```
pub fn run_simulation(
    backtest: &BacktestResult,
    config: MonteCarloConfig,
) -> Result<MonteCarloReport> {
    let trades = &backtest.trades;
    if trades.is_empty() {
        return Err(anyhow::anyhow!("No trades to simulate"));
    }

    let pnls: Vec<f64> = trades.iter().map(|t| t.pnl).collect();
    let horizon = config.horizon.unwrap_or(trades.len());
    let initial_capital = config.initial_capital.unwrap_or(backtest.initial_capital);
    let iterations = config.iterations;

    let mut final_equities = Vec::with_capacity(iterations);
    let mut max_drawdowns = Vec::with_capacity(iterations);
    let mut ruins = 0;

    let mut rng = rand::thread_rng();

    for _ in 0..iterations {
        let mut equity = initial_capital;
        let mut max_equity = initial_capital;
        let mut max_dd = 0.0;
        let mut ruined = false;

        for _ in 0..horizon {
            // Sample a trade PnL
            let pnl = *pnls.choose(&mut rng).unwrap();
            equity += pnl;

            if equity > max_equity {
                max_equity = equity;
            }

            let dd = (max_equity - equity) / max_equity;
            if dd > max_dd {
                max_dd = dd;
            }

            if equity <= 0.0 {
                ruined = true;
                break;
            }
        }

        if ruined {
            ruins += 1;
            // For stats, we can record 0 or negative equity
        }

        final_equities.push(equity);
        max_drawdowns.push(max_dd);
    }

    final_equities.sort_by(|a, b| a.partial_cmp(b).unwrap());
    max_drawdowns.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let median_equity = final_equities[iterations / 2];
    let median_return = (median_equity - initial_capital) / initial_capital * 100.0;

    // 5th percentile (Worst Case)
    let idx_05 = (iterations as f64 * 0.05) as usize;
    let worst_case_equity = final_equities[idx_05];
    let worst_case_return = (worst_case_equity - initial_capital) / initial_capital * 100.0;

    // 95th percentile (Best Case)
    let idx_95 = (iterations as f64 * 0.95) as usize;
    let best_case_equity = final_equities[idx_95];
    let best_case_return = (best_case_equity - initial_capital) / initial_capital * 100.0;

    // Median Max Drawdown
    let median_dd = max_drawdowns[iterations / 2] * 100.0;

    // VaR 95 (Loss at 5th percentile)
    // VaR is usually expressed as absolute loss or % loss from initial.
    // Here we report % loss of capital.
    // If worst case equity is > initial, VaR is 0 (profit).
    // If worst case equity < initial, VaR is (Initial - Worst) / Initial.
    let var_95 = if worst_case_equity < initial_capital {
        (initial_capital - worst_case_equity) / initial_capital * 100.0
    } else {
        0.0
    };

    Ok(MonteCarloReport {
        iterations,
        horizon,
        baseline_return_pct: backtest.metrics.total_return_pct,
        median_return_pct: median_return,
        best_case_return_pct: best_case_return,
        worst_case_return_pct: worst_case_return,
        risk_of_ruin_pct: (ruins as f64 / iterations as f64) * 100.0,
        max_drawdown_median_pct: median_dd,
        var_95_pct: var_95,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::{BacktestMetrics, BacktestTrade};

    fn create_mock_backtest(pnls: Vec<f64>, initial: f64) -> BacktestResult {
        let trades: Vec<BacktestTrade> = pnls
            .iter()
            .enumerate()
            .map(|(i, &p)| BacktestTrade {
                id: i.to_string(),
                entry_time: 0,
                exit_time: 0,
                side: "long".to_string(),
                qty: 1.0,
                entry_price: 100.0,
                exit_price: 100.0 + p,
                pnl: p,
                pnl_pct: p / 100.0,
                exit_reason: "test".to_string(),
            })
            .collect();

        BacktestResult {
            strategy: "Test".to_string(),
            symbol: "TEST".to_string(),
            timeframe: "1d".to_string(),
            initial_capital: initial,
            final_equity: initial + pnls.iter().sum::<f64>(),
            metrics: BacktestMetrics {
                total_return_pct: 0.0,
                cagr: 0.0,
                max_drawdown_pct: 0.0,
                win_rate: 0.0,
                profit_factor: 0.0,
                total_trades: trades.len(),
                winning_trades: 0,
                losing_trades: 0,
            },
            trades,
            equity_curve: vec![],
        }
    }

    #[test]
    fn test_monte_carlo_guaranteed_ruin() {
        // Strategy: Lose 1000 every time. Capital: 1000.
        // Horizon: 2 trades. Guaranteed ruin.
        let bt = create_mock_backtest(vec![-1000.0], 1000.0);
        let config = MonteCarloConfig {
            iterations: 100,
            horizon: Some(2),
            initial_capital: Some(1000.0),
        };

        let report = run_simulation(&bt, config).unwrap();
        assert_eq!(report.risk_of_ruin_pct, 100.0);
    }

    #[test]
    fn test_monte_carlo_growth() {
        // Strategy: Gain 10 every time.
        let bt = create_mock_backtest(vec![10.0], 1000.0);
        let config = MonteCarloConfig {
            iterations: 100,
            horizon: Some(10),
            initial_capital: Some(1000.0),
        };

        let report = run_simulation(&bt, config).unwrap();
        assert_eq!(report.risk_of_ruin_pct, 0.0);
        // 10 trades * 10 = 100 profit. 10% return.
        assert!((report.median_return_pct - 10.0).abs() < 0.1);
    }
}
