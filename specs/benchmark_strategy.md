# Feature Specification: Benchmark Strategies

**Status:** Proposed
**Author:** Vantage (Product Manager)
**Date:** 2026-02-26

## 1. User Story

**As a** Quantitative Trader,
**I want to** compare the performance of all available trading strategies on a specific historical dataset (BarSeries),
**So that** I can empirically select the best-performing strategy for the current market regime rather than relying on static rules or manual testing.

## 2. Context & Problem Statement

Currently, the `thales-cli` allows backtesting a *single* strategy via the `backtest` command. To compare strategies, a user must manually run the command multiple times and collate the results. The `execute_cycle.py` script relies on hardcoded heuristics to select strategies based on regime labels, which may not reflect actual performance.

There is a need for a unified `benchmark` command that runs a "tournament" of strategies against a dataset and ranks them.

## 3. Goals

*   **Efficiency**: Run all strategies in one command.
*   **Empiricism**: Rank strategies by objective metrics (e.g., Total Return, Win Rate, Drawdown).
*   **Maintainability**: Centralize strategy configuration to ensure the benchmark uses the same logic as the signal generator.

## 4. Acceptance Criteria

### 4.1. CLI Command
- A new command `benchmark` is available in `thales-cli`.
- Arguments:
    - `--input <path>`: Path to a JSON file containing `BarSeries`.
    - `--initial-capital <amount>`: Starting capital (default: 10000).
    - `--risk <amount>`: Risk per trade (default: 100).
    - `--sort-by <metric>`: Metric to sort results by (default: `total_return`). Supported: `total_return`, `win_rate`, `drawdown`.

### 4.2. Output Format
- The command outputs a JSON envelope containing a `BenchmarkReport`.
- The report includes:
    - `symbol`: The symbol tested.
    - `timeframe`: The timeframe tested.
    - `results`: A list of strategy results, sorted by the specified metric.
    - Each result includes:
        - `strategy`: Strategy name.
        - `metrics`: { `total_return_pct`, `max_drawdown_pct`, `win_rate`, `total_trades` }.

### 4.3. Strategy Coverage
- The benchmark must include all strategies currently available in the system:
    - BollingerBandsMeanReversion
    - EmaCrossover
    - RsiMeanReversion
    - Macd
    - Supertrend
    - DonchianBreakout
    - ParabolicSar
    - KeltnerChannelBreakout
    - StochasticOscillator
    - AdxMomentum
    - IchimokuCloud

## 5. Implementation Details (Technical Notes)

- **Strategy Factory**: A new `strategy_factory` module will be created to centralize strategy instantiation and configuration. This removes code duplication between `signals.rs` and `backtest.rs`.
- **Reuse Backtest Logic**: The `benchmark` command will reuse the existing `backtest::run_backtest` function to ensure consistency.
- **Performance**: Strategies will be run sequentially (or concurrently if feasible with `tokio`, though sequential is acceptable for V1).

## 6. Out of Scope

- **Optimization**: This feature runs strategies with *default* parameters. Parameter optimization (grid search) is a separate feature.
- **Visuals**: No charts or PDF reports in V1. JSON output is sufficient.
