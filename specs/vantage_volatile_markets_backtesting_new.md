# 🔭 Vantage: Spec for Volatile Markets Backtesting

## User Story
As a Trader, I want to backtest against volatile markets, so that I can validate my strategy's robustness under extreme conditions.

## So What?
Traders need to know how their strategies perform under extreme market conditions to prevent significant capital loss during black swan events.

## Metric Definition
Success = The backtesting engine successfully processes a dataset with missing or NaN values without crashing, and outputs the resulting metrics within 5 seconds.

## Gap Analysis
The current backtesting engine panics when encountering NaN values and does not support exporting results to CSV for external analysis.

## Acceptance Criteria
- Must handle NaN data without panicking.
- Must output a CSV report.

## Out of Scope
Real-time execution (Phase 2).
