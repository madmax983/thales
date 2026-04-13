# 🔭 Vantage: Spec for Backtesting Engine

## 👤 User Story
As a Quantitative Trader, I want to backtest my strategies against historical data so that I can verify their profitability and risk profile before allocating real capital.

## 💡 So What?
Currently, `thales-cli` allows users to fetch data and execute trades, but it lacks a native way to *verify* if a strategy is actually profitable over time. Users are forced to "test in production" (even if paper trading), which is slow and risky. By providing a local, fast backtesting engine that simulates strategy performance on historical data, we transform `thales-cli` from a simple execution bot into a robust research platform. This increases user confidence and drives adoption.

## 📈 Metric Definition
Success = Must process 1 year of 1-minute data (approx 525k bars) in under 5 seconds. PnL calculations must accurately account for spread and fees.

## 🔍 Gap Analysis
Currently, the `generate-signals` command exists but filters output to only show the *latest* signal for live execution, discarding historical signals. There is no native capability to evaluate a strategy's performance over an entire dataset. We need a new command `backtest` that iterates through all historical signals, simulates execution (fills, PnL tracking), and generates a performance report.

## ✅ Acceptance Criteria
- New subcommand `backtest` must accept input market data, strategy name, initial balance, and fee rate.
- Must execute the strategy over the full dataset and simulate trades chronologically.
- Must respect Stop Loss (SL) and Take Profit (TP) levels attached to signals if price data supports it.
- Must output a summary report containing Total Return, Win Rate, Profit Factor, Max Drawdown, and Trade Count.
- Must handle edge cases (empty data, zero signals, bankruptcy) gracefully without panicking.

## 🚫 Out of Scope
- Strategy parameter optimization ("Grid Search").
- Visualizations (Generating charts/graphs).
- Complex variable slippage models.
- Multi-asset portfolio backtesting (single asset only for Phase 1).
