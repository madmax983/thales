
# Trading Strategy: KDJ Indicator Strategy

## Strategy Specification

**Name:** KdjIndicatorStrategy

**Description:** A mean-reversion and momentum strategy based on the KDJ indicator. It utilizes the fast %K line, slow %D line, and highly sensitive divergence %J line to automatically identify overbought and oversold conditions and capture trend reversals before they fully materialize.

**Rationale:** Traditional momentum oscillators can lag, leading to delayed entries. The KDJ indicator introduces the %J line, acting as a leading indicator to enable earlier entries on reversals. This is particularly useful for identifying immediate market turning points in choppy or turning markets.

## Requirements

### Implementation Details
- Uses Polars for fast vector operations and data analysis.
- Implements the `Strategy` trait in Rust.
- Uses native `kdj` and `atr` indicators.

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry (Buy):** %J line crosses above 0 (oversold reversal) OR %K crosses above %D while both are below 20 (oversold threshold).
- **Short Entry (Sell):** %J line crosses below 100 (overbought reversal) OR %K crosses below %D while both are above 80 (overbought threshold).

### Exit Conditions
- **Long Exit (Sell):** %J line crosses above 100 OR %K crosses below %D.
- **Short Exit (Buy):** %J line crosses below 0 OR %K crosses above %D.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** Strictly fixed allocation bounded by `max_position_size` for entry, "max" for exits.

### Expected Backtesting Metrics
- **Expected Win Rate:** > 50% (High sensitivity allows early entry, though false signals are possible in strong trends).
- **Expected Sharpe Ratio:** > 1.0
- **Max Drawdown:** < 20%
