
---

# Trading Strategy: Ichimoku Cloud Breakout

## Strategy Specification

**Name:** IchimokuCloud

**Description:** A comprehensive trend-following strategy using the Ichimoku Kinko Hyo system. It generates buy signals when the price breaks above the Kumo (Cloud) with Bullish Tenkan-Kijun crossover and Chikou Span confirmation.

**Rationale:** The Ichimoku Cloud provides a holistic view of the market, identifying support/resistance, trend direction, and momentum in a single glance. Trading breakouts from the cloud with multiple confirmations reduces false signals.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `ichimoku` indicator for component calculation and `atr` for stop loss.

### Strategy Type
Trend Following / Breakout

### Entry Conditions
- **Long Entry (Buy):**
    1. Close Price > Cloud Top (Max(Span A, Span B)).
    2. Tenkan-sen > Kijun-sen (Bullish Cross).
    3. Tenkan-sen (Previous) <= Kijun-sen (Previous) (Cross event).
    4. Close Price > Close Price 26 periods ago (Chikou Confirmation).

- **Short Entry (Sell):**
    1. Close Price < Cloud Bottom (Min(Span A, Span B)).
    2. Tenkan-sen < Kijun-sen (Bearish Cross).
    3. Tenkan-sen (Previous) >= Kijun-sen (Previous) (Cross event).
    4. Close Price < Close Price 26 periods ago (Chikou Confirmation).

### Exit Conditions
- **Long Exit (Sell):** Close Price < Kijun-sen OR Close Price < Cloud Top.
- **Short Exit (Buy):** Close Price > Kijun-sen OR Close Price > Cloud Bottom.
- **Stop Loss:** Kijun-sen (Dynamic) or Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units). Actual sizing is handled by risk management layer.

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{ichimoku, atr};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct IchimokuCloud {
    config: IchimokuCloudConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct IchimokuCloudConfig {
    pub tenkan_period: usize,
    pub kijun_period: usize,
    pub senkou_b_period: usize,
    pub displacement: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss as a hard stop, but Kijun-sen acts as a trailing stop/exit signal.
- **Confirmation:** Requires multiple components (Price, Cloud, TK Cross, Chikou) to align, reducing trade frequency but increasing probability.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `senkou_b_period` + `displacement`.

### Performance
- Rolling Max/Min calculation is O(N).
- Signal generation loop is O(N).
