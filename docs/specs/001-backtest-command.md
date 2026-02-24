# SPEC-001: Backtesting Engine

**Status**: Draft
**Owner**: Vantage (Product Manager)
**Created**: 2024-05-22

## 1. User Story
> "As a Quantitative Trader, I want to backtest my strategies against historical data so that I can verify their profitability and risk profile before allocating real capital."

## 2. The "So What?" (Business Value)
Currently, `thales-cli` allows users to fetch data and execute trades, but it lacks a native way to *verify* if a strategy is actually profitable over time.
- **Problem**: Users are forced to "test in production" (even if paper trading), which is slow and risky.
- **Solution**: A local, fast backtesting engine that simulates strategy performance on historical data.
- **Value**: Transforms `thales-cli` from a simple execution bot into a robust research platform. Increases user confidence and adoption.

## 3. Gap Analysis
- **Current State**: `generate-signals` command exists but filters output to only show the *latest* signal for live execution. It discards historical signals.
- **Desired State**: A new command `backtest` that iterates through *all* historical signals, simulates execution (fills, PnL tracking), and generates a performance report.

## 4. Metrics & Success Criteria
- **Performance**: Must process 1 year of 1-minute data (approx 525k bars) in under 5 seconds.
- **Accuracy**: PnL calculations must account for spread and fees (configurable).
- **Usability**: Command line interface must be intuitive and consistent with existing commands.

## 5. Specification

### 5.1 CLI Command
New subcommand: `backtest`

```bash
thales-cli backtest \
  --input <path-to-market-data.json> \
  --strategy <StrategyName> \
  --initial-balance <Float> (Default: 10000.0) \
  --fee-rate <Float> (Default: 0.001) \
  --output <json|text>
```

### 5.2 Inputs
- **Market Data**: Standard `BarSeries` JSON (output from `fetch-market-data`).
- **Strategy Config**: Standard strategy configuration (defaults loaded by name, overrides via file optional).

### 5.3 Execution Logic (Simulation)
1.  **Initialize**: Set `Cash = Initial Balance`, `Position = 0`.
2.  **Generate Signals**: Run the selected strategy on the full dataset to get a vector of `Signal`s.
3.  **Iterate**: Walk through signals in chronological order.
4.  **Simulate Trade**:
    - **Entry**: If `Signal == Entry` and `Cash > 0`:
        - Calculate Size (based on strategy logic or default risk).
        - Update `Cash` (subtract Cost + Fees).
        - Update `Position` (add Quantity).
        - Log Trade (Entry Price, Time).
    - **Exit**: If `Signal == Exit` and `Position > 0`:
        - Calculate Revenue (Price * Quantity).
        - Update `Cash` (add Revenue - Fees).
        - Update `Position` (set to 0).
        - Log Trade (Exit Price, Time, PnL).
5.  **Stop Loss / Take Profit**: The engine must respect SL/TP levels attached to signals if the price data supports it (i.e., Low < SL or High > TP).

### 5.4 Output (Report)
The command must output a summary containing:
- **Total Return**: (Final Equity - Initial Balance) / Initial Balance
- **Win Rate**: (Winning Trades / Total Trades)
- **Profit Factor**: (Gross Profit / Gross Loss)
- **Max Drawdown**: Maximum peak-to-valley decline in equity.
- **Trade Count**: Total number of executed trades.

## 6. Acceptance Criteria
1.  [ ] **Command Exists**: `thales-cli backtest --help` works.
2.  [ ] **Data Ingestion**: Accepts standard `BarSeries` JSON.
3.  [ ] **Strategy Execution**: Runs `BollingerBands`, `RsiMeanReversion`, etc.
4.  [ ] **Reporting**: Outputs correct JSON/Text summary of PnL.
5.  [ ] **Edge Cases**: Handles empty data, zero signals, and bankruptcy (Cash <= 0) gracefully without panicking.

## 7. Out of Scope (Phase 1)
- **Optimization**: "Grid Search" for strategy parameters.
- **Visualizations**: Generating charts/graphs (use external tools like Python/Matplotlib for now).
- **Complex Slippage**: Variable slippage models.
- **Multi-Asset**: Portfolio backtesting (single asset only for now).
