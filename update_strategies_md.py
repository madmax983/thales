import re
with open("strategies.md", "a") as f:
    f.write('''
---

# Trading Strategy: ROC Momentum

## Strategy Specification

**Name:** RocMomentum

**Description:** A momentum strategy based on the Rate of Change (ROC) indicator. It measures the percentage change between the current price and the price a certain number of periods ago.

**Rationale:** Momentum often leads price changes. An upward cross of the ROC zero-line indicates increasing bullish momentum, while a downward cross indicates bearish momentum.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses inline ROC calculation and the `atr` indicator for stop loss calculation.

### Strategy Type
Momentum

### Entry Conditions
- **Long Entry (Buy):** ROC crosses ABOVE 0.0.

### Exit Conditions
- **Long Exit (Sell):** ROC crosses BELOW 0.0.
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::atr;
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct RocMomentum {
    config: RocMomentumConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RocMomentumConfig {
    pub roc_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to current market volatility.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `roc_period` + 1.

### Performance
- ROC calculation is O(N).
- Signal generation loop is O(N).
''')
