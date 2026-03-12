# 🔭 Vantage: Spec for Volatile Market Backtesting

## User Story
As a Trader, I want to backtest against volatile markets, so that I can ensure my trading strategies do not fail or lose significant capital during market crashes or extreme volatility events.

## So What?
If a trader cannot backtest their strategies against highly volatile markets, they are exposed to catastrophic risk when black swan events occur. By enabling backtesting on volatile datasets, we give our users the confidence to deploy capital, increasing trust in our platform and ensuring strategy robustness.

## Metric Definition
Success = The backtesting engine successfully processes a high-volatility dataset containing NaN values without panicking, and successfully generates a CSV output report in 100% of runs.

## Gap Analysis
Currently, the `thales-cli backtest` command assumes clean, continuous data and lacks the ability to handle missing data points (NaNs) gracefully. Competing platforms handle data irregularities and provide stress-testing capabilities natively. We need to add resilience to our data processing layer so that users can stress-test their algorithms against extreme market conditions.

## Acceptance Criteria
- Must handle NaN data without panicking.
- Must output a CSV report.

## Out of Scope
- Real-time execution (Phase 2).
