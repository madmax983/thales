use crate::backtest::{self, BacktestConfig, BacktestMetrics};
use crate::strategy_factory;
use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub initial_capital: f64,
    pub risk_per_trade: f64,
    pub sort_by: String, // "total_return", "win_rate", "drawdown"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub symbol: String,
    pub timeframe: String,
    pub results: Vec<BenchmarkEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkEntry {
    pub strategy: String,
    pub metrics: BacktestMetrics,
}

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
