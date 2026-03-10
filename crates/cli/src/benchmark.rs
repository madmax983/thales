//! Provides a framework to benchmark all available strategies against a single asset.
//!
//! This module iterates through every trading strategy registered in the
//! [`strategy_factory`] and runs a standard backtest against the provided [`BarSeries`].
//! It then compiles the results and sorts them based on user-defined criteria (e.g., win rate, total return).
//!
//! # Examples
//!
//! ```no_run
//! use thales_cli::benchmark::{run_benchmark, BenchmarkConfig};
//! use contracts::{BarSeries, Bar};
//!
//! #[tokio::main]
//! async fn main() {
//!     let bars = BarSeries {
//!         schema_version: "v0".to_string(),
//!         bars: vec![], // Populate with historical data
//!     };
//!
//!     let config = BenchmarkConfig {
//!         initial_capital: 10_000.0,
//!         risk_per_trade: 100.0,
//!         sort_by: "win_rate".to_string(),
//!     };
//!
//!     // Benchmark all strategies against the dataset
//!     let report = run_benchmark(&bars, config).await.unwrap();
//!
//!     if let Some(top_strategy) = report.results.first() {
//!         println!("Top Strategy: {}", top_strategy.strategy);
//!         println!("Win Rate: {:.2}%", top_strategy.metrics.win_rate * 100.0);
//!     }
//! }
//! ```

use crate::backtest::{self, BacktestConfig, BacktestMetrics};
use crate::strategy_factory;
use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

/// Configuration for the benchmarking suite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// The starting account balance for each backtest.
    pub initial_capital: f64,
    /// The maximum dollar amount to risk per trade.
    pub risk_per_trade: f64,
    /// The metric by which to sort the benchmark results.
    /// Accepted values: `"total_return"`, `"win_rate"`, `"drawdown"`.
    pub sort_by: String,
}

/// The final report containing the benchmark results for all evaluated strategies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    /// The asset symbol that was benchmarked.
    pub symbol: String,
    /// The timeframe of the data used (e.g., "1d").
    pub timeframe: String,
    /// The sorted list of strategy performances.
    pub results: Vec<BenchmarkEntry>,
}

/// The performance metrics for a single strategy within the benchmark report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkEntry {
    /// The name of the strategy.
    pub strategy: String,
    /// The backtest performance metrics.
    pub metrics: BacktestMetrics,
}

/// Benchmarks all available strategies against a given dataset.
///
/// This function retrieves the list of all registered strategies, runs a backtest
/// for each using the provided `config`, and returns a consolidated report sorted
/// by the specified metric.
///
/// # Arguments
///
/// * `bars` - The historical data series to evaluate.
/// * `config` - Capital, risk, and sorting preferences.
///
/// # Errors
///
/// Returns an error if the `bars` series is empty. Individual strategy failures
/// during backtesting are caught and silently skipped.
pub async fn run_benchmark(bars: &BarSeries, config: BenchmarkConfig) -> Result<BenchmarkReport> {
    if bars.bars.is_empty() {
        return Err(anyhow::anyhow!("No bars provided for benchmark"));
    }

    let symbol = bars.bars[0].symbol.clone();
    let timeframe = bars.bars[0].timeframe.clone();

    let strategies = strategy_factory::list_strategies();
    let mut results = Vec::new();

    let backtest_config = BacktestConfig {
        initial_capital: config.initial_capital,
        risk_per_trade: config.risk_per_trade,
    };

    for strategy_name in strategies {
        // Run backtest
        // We reuse the same config for all strategies
        let result = backtest::run_backtest(bars, strategy_name, backtest_config.clone()).await;

        match result {
            Ok(bt_result) => {
                results.push(BenchmarkEntry {
                    strategy: strategy_name.to_string(),
                    metrics: bt_result.metrics,
                });
            }
            Err(e) => {
                eprintln!("Error benchmarking strategy {}: {}", strategy_name, e);
                // We skip failed strategies
            }
        }
    }

    // Sort results
    match config.sort_by.as_str() {
        "win_rate" => {
            results.sort_by(|a, b| {
                b.metrics
                    .win_rate
                    .partial_cmp(&a.metrics.win_rate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        "drawdown" => {
            // Lower drawdown is better, so sort ascending
            results.sort_by(|a, b| {
                a.metrics
                    .max_drawdown_pct
                    .partial_cmp(&b.metrics.max_drawdown_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        _ => {
            // total_return (default)
            results.sort_by(|a, b| {
                b.metrics
                    .total_return_pct
                    .partial_cmp(&a.metrics.total_return_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    Ok(BenchmarkReport {
        symbol,
        timeframe,
        results,
    })
}
