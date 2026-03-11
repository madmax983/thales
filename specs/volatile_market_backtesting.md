# 🔭 Vantage: Spec for Volatile Market Backtesting

## 👤 User Story
As a Trader, I want to backtest against volatile markets, so that I can validate my strategy's robustness under extreme conditions.

## 💡 So What?
Currently, strategies are only tested against typical market conditions. Without stress-testing against high volatility, traders risk significant capital loss when black swan events or sudden market shocks occur. Providing a tool to simulate these environments builds trust and prevents blow-ups.

## 📈 Metric Definition
Success = The backtesting engine successfully processes a 1-year high-volatility dataset (including simulated gaps and spikes) without crashing, and outputs the resulting metrics within 5 seconds.

## 🔍 Gap Analysis
The current `thales-cli backtest` command processes standard historical data but lacks mechanisms to inject synthetic volatility or specifically target historical high-volatility periods (e.g., flash crashes). Competitors often provide "stress test" suites that we currently lack.

## ✅ Acceptance Criteria
- Must handle NaN data without panicking.
- Must be able to process datasets with sudden, extreme price gaps.
- Must output a CSV report.

## 🚫 Out of Scope
Real-time execution (Phase 2).
