# 🔭 Vantage: Spec for Backtesting Engine

## 1. The "User Story"
**As a** Quantitative Trader,
**I want** to simulate my trading strategies against historical market data,
**So that** I can validate their profitability, risk profile, and robustness before risking real capital.

## 2. The "So What?" (Business Value)
Currently, the `thales-cli` toolkit allows users to fetch data, generate *current* signals, and execute them. However, it lacks a mechanism to verify if a strategy is actually viable.
*   **Problem:** Users are flying blind. They must deploy capital to test a strategy.
*   **Risk:** High probability of loss due to unverified logic or parameter overfitting.
*   **Solution:** A Backtesting Engine allows users to iterate rapidly, optimize parameters, and gain confidence in their systems.
*   **Metric:** Success = Users can run a backtest on 1 year of 1-minute data in < 5 seconds and receive a PnL report.

## 3. Acceptance Criteria

### ✅ Must Have (MVP)
1.  **CLI Command:** A new `backtest` command in `thales-cli`.
2.  **Historical Simulation:**
    *   Accepts a JSON file containing `BarSeries` (price data).
    *   Replays the data bar-by-bar (or vectorized equivalents where safe).
    *   Generates signals at each step using the specified `Strategy`.
3.  **Execution Simulation:**
    *   Simulates order execution based on signal type (`Entry`, `Exit`, `ScaleIn`, `ScaleOut`).
    *   **Assumptions:**
        *   Market Orders fill at the *next* bar's Open price (to avoid lookahead bias).
        *   Limit Orders fill if price touches the limit level.
        *   Stop Losses fill if price touches the stop level.
4.  **Portfolio Tracking:**
    *   Tracks Cash, Equity, and Positions over time.
    *   Handles long and short positions (if strategy supports it).
5.  **Reporting:**
    *   Outputs a JSON envelope containing:
        *   `metrics`: Total Return, Max Drawdown, Win Rate, Profit Factor, Sharpre Ratio (simplified).
        *   `trades`: List of all executed trades with Entry/Exit time and price.
        *   `equity_curve`: Time-series of account equity.

### 🚫 Out of Scope (Phase 1)
*   **Multi-Asset Backtesting:** The MVP will test one symbol at a time.
*   **Tick-Level Granularity:** We will use price data bars only.
*   **Complex Commission Models:** We will assume a flat fee or zero fee for now.
*   **Slippage Models:** We will assume perfect execution at the next Open (or trigger price).

## 4. Interface Design

### CLI Command
```bash
thales-cli backtest \
  --input <path-to-bars.json> \
  --strategy <StrategyName> \
  --initial-capital <float> \
  --risk-per-trade <float> \
  --output <json|markdown>
```

### Arguments
*   `--input`: Path to the JSON file containing `BarSeries` (output of `fetch-market-data`).
*   `--strategy`: Name of the strategy to test (e.g., `BollingerBandsMeanReversion`).
*   `--initial-capital`: Starting cash (default: 10,000).
*   `--risk-per-trade`: Capital to risk per trade (default: 100.0).
*   `--output`: Format of the output report.

## 5. Output Schema (JSON)

```json
{
  "strategy": "BollingerBandsMeanReversion",
  "symbol": "BTCUSD",
  "timeframe": "1h",
  "period_start": 1609459200000,
  "period_end": 1640995200000,
  "initial_capital": 10000.0,
  "final_equity": 12500.0,
  "metrics": {
    "total_return_pct": 25.0,
    "cagr": 25.0,
    "max_drawdown_pct": 10.5,
    "win_rate": 0.55,
    "profit_factor": 1.5,
    "total_trades": 150
  },
  "trades": [
    {
      "id": "1",
      "entry_time": 1609459200000,
      "exit_time": 1609545600000,
      "side": "long",
      "qty": 1.5,
      "entry_price": 30000.0,
      "exit_price": 31000.0,
      "pnl": 1500.0,
      "pnl_pct": 0.05
    }
    // ...
  ]
}
```

## 6. Implementation Notes (For Engineering)
*   **Reuse Strategy Trait:** The existing `Strategy` trait generates signals from a `DataFrame`.
*   **Vectorization vs. Iteration:**
    *   Since strategies return a *series* of signals (calculated on the whole history), the backtester can:
        1.  Run the strategy once on the full dataset to get all potential signals.
        2.  Iterate through the signals chronologically to simulate execution (managing state, stops, and capital).
    *   **Caution:** Ensure the strategy calculation does not leak future data (e.g., using `shift(-1)`). Most indicators (MA, RSI) are backward-looking, so this is safe if standard libraries are used.
*   **State Management:** The backtester needs a simple `Portfolio` struct to track cash and positions.
