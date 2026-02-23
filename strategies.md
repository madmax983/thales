# Trading Strategy: Bollinger Bands Mean Reversion

## Strategy Specification

**Name:** BollingerBandsMeanReversion

**Description:** A mean reversion strategy that uses Bollinger Bands to identify overbought and oversold conditions. It buys when the price closes below the lower band and sells when the price closes above the upper band.

**Rationale:** Asset prices tend to revert to the mean over time. Bollinger Bands provide a dynamic measure of volatility and relative price levels. Prices at the bands are considered relatively high or low.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses sliding window calculation for performance and simplicity.

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry (Buy):** Close Price < Lower Bollinger Band

### Exit Conditions
- **Long Exit (Sell):** Close Price > Upper Bollinger Band

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).
- In a real implementation, this would be dynamic based on portfolio value and risk limits.

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct BollingerBandsMeanReversion {
    config: BollingerBandsConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BollingerBandsConfig {
    pub window_size: usize,
    pub num_std_dev: f64,
    pub stop_loss_pct: f64,
    pub symbol: String,
}
// ... implementation details ...
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** The configuration includes `stop_loss_pct`, although the current signal logic mainly focuses on band crossovers. A complete implementation should generate exit signals if price drops by X% from entry.
- **Position Size:** Currently returns a hint. The execution engine must validate this against account limits.

### Backtesting Requirements
- The strategy logic accepts a `DataFrame` containing historical data, making it fully backtestable.
- Ensure data quality (no gaps, correct timestamps).

### Performance
- Uses O(N) sliding window calculation for SMA and Standard Deviation.
- Efficient for typical market data window sizes.
