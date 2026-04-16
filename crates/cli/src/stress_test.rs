//! Stress Testing Strategy Resilience
//!
//! This module orchestrates the evaluation of a trading strategy under extreme, synthetically generated
//! market conditions (Black Swan events). It runs the strategy through a standard historical backtest
//! and compares the performance against a "stressed" version of the same market data, quantifying the strategy's
//! robustness to unexpected shocks.
//!
//! # Core Concepts
//! - **Baseline vs. Stress:** Runs two parallel backtests (one normal, one mutated) and returns a side-by-side comparison.
//! - **Black Swan Injection:** Uses the `black_swan` module to dynamically mutate the input data with events like flash crashes or liquidity freezes.
//!
//! # Examples
//!
//! ```no_run
//! use contracts::BarSeries;
//! use thales_cli::stress_test::run_stress_test;
//! use thales_cli::black_swan::{BlackSwanConfig, BlackSwanEvent};
//! use thales_cli::backtest::BacktestConfig;
//!
//! # async fn example() {
//! // Assume `series` is populated
//! # let series = BarSeries { schema_version: "v1".to_string(), bars: vec![] };
//! let swan_config = BlackSwanConfig {
//!     event: BlackSwanEvent::FlashCrash { drop_pct: 0.30, duration_bars: 3 },
//!     start_index: 10,
//!     seed: None,
//! };
//!
//! let backtest_config = BacktestConfig {
//!     initial_capital: 10000.0,
//!     risk_per_trade: 0.02,
//! };
//!
//! let result = run_stress_test(&series, "BollingerBands", backtest_config, swan_config).await.unwrap();
//! println!("Original PnL: {}", result.original_result.unwrap().metrics.total_return_pct);
//! println!("Stressed PnL: {}", result.stressed_result.unwrap().metrics.total_return_pct);
//! # }
//! ```

use anyhow::Result;
use contracts::BarSeries;
use serde::{Deserialize, Serialize};

use crate::backtest::{BacktestConfig, BacktestResult};
use crate::black_swan::BlackSwanConfig;

/// A summary of stress test execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestResult {
    /// Result from the original backtest
    pub original_result: Option<BacktestResult>,
    /// Result from the stressed backtest
    pub stressed_result: Option<BacktestResult>,
}

/// Run a stress test on a strategy by simulating a Black Swan event
pub async fn run_stress_test(
    series: &BarSeries,
    strategy_name: &str,
    backtest_config: BacktestConfig,
    black_swan_config: BlackSwanConfig,
) -> Result<StressTestResult> {
    // Run original backtest
    let original_result =
        crate::backtest::run_backtest(series, strategy_name, backtest_config.clone()).await?;

    // Inject Black Swan event
    let stressed_series = crate::black_swan::inject_black_swan(series, black_swan_config)?;

    // Run stressed backtest
    let stressed_result =
        crate::backtest::run_backtest(&stressed_series, strategy_name, backtest_config).await?;

    Ok(StressTestResult {
        original_result: Some(original_result),
        stressed_result: Some(stressed_result),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::black_swan::BlackSwanEvent;
    use contracts::Bar;

    #[tokio::test]
    async fn test_run_stress_test_failure() {
        let bars = BarSeries {
            schema_version: "v0".to_string(),
            bars: vec![Bar {
                symbol: "TEST".to_string(),
                market: "equities".to_string(),
                timeframe: "1m".to_string(),
                timestamp_unix_ms: 1000,
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 100.0,
                volume: 1000.0,
            }],
        };

        let backtest_config = BacktestConfig {
            initial_capital: 10000.0,
            risk_per_trade: 100.0,
        };

        let black_swan_config = BlackSwanConfig {
            event: BlackSwanEvent::FlashCrash {
                drop_pct: 0.30,
                duration_bars: 2,
            },
            start_index: 0,
            seed: None,
        };

        let result = run_stress_test(
            &bars,
            "RsiMeanReversion",
            backtest_config,
            black_swan_config,
        )
        .await;

        // Ensure that it's actually implemented
        assert!(result.is_ok(), "Stress test is not implemented");
    }
}
