//! Strategy Correlation Analysis Module
//!
//! Provides functionality to backtest multiple strategies on a single asset and
//! compute the Pearson correlation matrix of their equity curve returns.
//! This is useful for building diversified ensembles of strategies that don't
//! all fail or succeed at the exact same time.
//!
//! # Core Concepts
//!
//! - **Correlation Matrix**: A table showing correlation coefficients between strategies.
//! - **Diversification**: If two strategies have a correlation near 1.0, they are redundant.
//!   If they are near 0.0 or negative, they provide diversification.

use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

use crate::backtest::{self, BacktestConfig};
use crate::strategy_factory;

/// Configuration for Strategy Correlation Analysis.
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::strategy_correlation::CorrelationConfig;
///
/// let config = CorrelationConfig {
///     initial_capital: 10000.0,
///     risk_per_trade: 100.0,
///     strategies: vec!["SmaCrossover".to_string(), "EmaCrossover".to_string()],
/// };
///
/// assert_eq!(config.strategies.len(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationConfig {
    /// Starting capital for each strategy backtest.
    pub initial_capital: f64,
    /// Risk allowed per trade during backtest.
    pub risk_per_trade: f64,
    /// The names of the strategies to backtest and correlate. If empty, all active strategies are used.
    pub strategies: Vec<String>,
}

/// The result of a Strategy Correlation Analysis run.
///
/// # Examples
///
/// ```rust
/// use thales_cli::experimental::strategy_correlation::CorrelationReport;
///
/// let report = CorrelationReport {
///     symbol: "AAPL".to_string(),
///     strategies: vec!["SmaCrossover".to_string(), "EmaCrossover".to_string()],
///     correlation_matrix: vec![
///         vec![1.0, 0.85],
///         vec![0.85, 1.0],
///     ],
/// };
///
/// assert_eq!(report.correlation_matrix[0][1], 0.85);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationReport {
    /// The symbol analyzed.
    pub symbol: String,
    /// The strategies that successfully produced equity curves.
    pub strategies: Vec<String>,
    /// An N x N matrix of Pearson correlation coefficients.
    pub correlation_matrix: Vec<Vec<f64>>,
}

/// Computes the Pearson correlation coefficient between two slices of f64.
/// Both slices must be the same length.
fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
    if x.is_empty() || y.is_empty() || x.len() != y.len() {
        return 0.0;
    }

    let n = x.len() as f64;
    let sum_x: f64 = x.iter().sum();
    let sum_y: f64 = y.iter().sum();
    let sum_x_sq: f64 = x.iter().map(|&val| val * val).sum();
    let sum_y_sq: f64 = y.iter().map(|&val| val * val).sum();
    let sum_xy: f64 = x
        .iter()
        .zip(y.iter())
        .map(|(&val_x, &val_y)| val_x * val_y)
        .sum();

    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_x_sq - sum_x * sum_x) * (n * sum_y_sq - sum_y * sum_y)).sqrt();

    if denominator == 0.0 {
        return 0.0;
    }
    numerator / denominator
}

/// Backtests a list of strategies and computes the correlation of their returns.
///
/// The function runs a backtest for each requested strategy. It then computes the
/// period-to-period returns of each strategy's equity curve and calculates a
/// Pearson correlation matrix comparing every strategy against every other strategy.
///
/// # Errors
///
/// Returns an error if the `BarSeries` is empty.
///
/// # Examples
///
/// ```rust
/// use contracts::{BarSeries, Bar};
/// use thales_cli::experimental::strategy_correlation::{analyze_correlations, CorrelationConfig};
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let mut bars = Vec::new();
/// for i in 0..50 {
///     bars.push(Bar { symbol: "TEST".into(), market: "equities".into(), timeframe: "1d".into(), timestamp_unix_ms: i * 1000, open: 100.0, high: 105.0, low: 95.0, close: 100.0 + i as f64, volume: 100.0 });
/// }
///
/// let series = BarSeries { schema_version: "v0".to_string(), bars };
/// let config = CorrelationConfig {
///     initial_capital: 10000.0,
///     risk_per_trade: 100.0,
///     strategies: vec!["SmaCrossover".to_string(), "EmaCrossover".to_string()],
/// };
///
/// let report = analyze_correlations(&series, config).await.unwrap();
/// assert_eq!(report.strategies.len(), 2);
/// assert_eq!(report.correlation_matrix.len(), 2);
/// # });
/// ```
pub async fn analyze_correlations(
    series: &BarSeries,
    config: CorrelationConfig,
) -> Result<CorrelationReport> {
    if series.bars.is_empty() {
        return Err(anyhow::anyhow!("No bars provided for correlation analysis"));
    }

    let symbol = series.bars[0].symbol.clone();

    let strategies_to_test = if config.strategies.is_empty() {
        strategy_factory::list_strategies()
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
    } else {
        config.strategies.clone()
    };

    let backtest_config = BacktestConfig {
        initial_capital: config.initial_capital,
        risk_per_trade: config.risk_per_trade,
    };

    let mut successful_strategies = Vec::new();
    let mut strategy_returns = Vec::new();

    for strategy_name in &strategies_to_test {
        match backtest::run_backtest(series, strategy_name, backtest_config.clone()).await {
            Ok(bt_result) => {
                let eq_curve = bt_result.equity_curve;
                if eq_curve.len() < 2 {
                    continue; // Not enough data points to compute returns
                }

                let mut returns = Vec::with_capacity(eq_curve.len() - 1);
                for i in 1..eq_curve.len() {
                    let prev = eq_curve[i - 1].equity;
                    let curr = eq_curve[i].equity;
                    let ret = if prev > 0.0 {
                        (curr - prev) / prev
                    } else {
                        0.0
                    };
                    returns.push(ret);
                }

                successful_strategies.push(strategy_name.clone());
                strategy_returns.push(returns);
            }
            Err(e) => {
                eprintln!("Skipping {} due to error: {}", strategy_name, e);
            }
        }
    }

    let num_strats = successful_strategies.len();
    let mut correlation_matrix = vec![vec![0.0; num_strats]; num_strats];

    for i in 0..num_strats {
        for j in 0..num_strats {
            if i == j {
                correlation_matrix[i][j] = 1.0;
            } else if i < j {
                let corr = pearson_correlation(&strategy_returns[i], &strategy_returns[j]);
                correlation_matrix[i][j] = corr;
                correlation_matrix[j][i] = corr; // Matrix is symmetric
            }
        }
    }

    Ok(CorrelationReport {
        symbol,
        strategies: successful_strategies,
        correlation_matrix,
    })
}

/// Prints a simple ASCII heatmap/matrix of the correlations
pub fn print_ascii_correlations(report: &CorrelationReport) {
    if report.strategies.is_empty() {
        println!("No strategies were successfully backtested for correlation.");
        return;
    }

    println!(
        "\n=== Strategy Correlation Matrix for {} ===",
        report.symbol
    );

    // Header row
    print!("{:>20} |", "");
    for i in 0..report.strategies.len() {
        // Just print an ID to save space, and a legend later
        print!(" {:>5} |", format!("S{}", i));
    }
    println!();

    let mut separator = "-".repeat(22);
    for _ in 0..report.strategies.len() {
        separator.push_str("-------+");
    }
    println!("{}", separator);

    // Matrix rows
    for i in 0..report.strategies.len() {
        // Truncate name if too long
        let mut name = report.strategies[i].clone();
        if name.len() > 18 {
            name.truncate(15);
            name.push_str("...");
        }

        print!("{:>20} |", name);
        for j in 0..report.strategies.len() {
            let val = report.correlation_matrix[i][j];

            // Basic color coding:
            // Negative/Low = Green (good for diversification)
            // High Positive = Red (redundant)
            let (color_start, color_end) = if i == j {
                ("\x1b[1;30m", "\x1b[0m") // Dark Gray for self-correlation
            } else if val > 0.7 {
                ("\x1b[1;31m", "\x1b[0m") // Red
            } else if val < 0.3 {
                ("\x1b[1;32m", "\x1b[0m") // Green
            } else {
                ("\x1b[1;33m", "\x1b[0m") // Yellow
            };

            print!(" {}{:5.2}{}", color_start, val, color_end);
            print!(" |");
        }
        println!();
    }
    println!("{}", separator);

    // Legend
    println!("Legend:");
    for (i, name) in report.strategies.iter().enumerate() {
        println!("  S{}: {}", i, name);
    }
    println!("  < 0.3: Good Diversification");
    println!("  > 0.7: High Redundancy");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Bar;

    fn create_dummy_bar(close: f64, ts: i64) -> Bar {
        Bar {
            symbol: "TEST".to_string(),
            market: "equities".to_string(),
            timeframe: "1d".to_string(),
            timestamp_unix_ms: ts,
            open: close,
            high: close,
            low: close,
            close,
            volume: 100.0,
        }
    }

    #[test]
    fn test_pearson_correlation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0]; // Perfect positive
        let z = vec![5.0, 4.0, 3.0, 2.0, 1.0]; // Perfect negative

        let corr_xy = pearson_correlation(&x, &y);
        let corr_xz = pearson_correlation(&x, &z);

        assert!((corr_xy - 1.0).abs() < 1e-6);
        assert!((corr_xz - (-1.0)).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_analyze_correlations() {
        // Create a fake trending market
        let mut bars = Vec::new();
        for i in 0..50 {
            bars.push(create_dummy_bar(100.0 + (i as f64 * 2.0), i * 1000));
        }

        let series = BarSeries {
            schema_version: "v0".to_string(),
            bars,
        };

        let config = CorrelationConfig {
            initial_capital: 10000.0,
            risk_per_trade: 100.0,
            strategies: vec!["SmaCrossover".to_string(), "EmaCrossover".to_string()],
        };

        let report = analyze_correlations(&series, config).await.unwrap();

        assert_eq!(report.strategies.len(), 2);
        assert_eq!(report.correlation_matrix.len(), 2);

        // Self-correlation must be 1.0
        assert!((report.correlation_matrix[0][0] - 1.0).abs() < 1e-6);
        assert!((report.correlation_matrix[1][1] - 1.0).abs() < 1e-6);

        // Matrix should be symmetric
        assert_eq!(
            report.correlation_matrix[0][1],
            report.correlation_matrix[1][0]
        );
    }
}
