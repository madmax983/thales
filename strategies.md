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

---

# Trading Strategy: EMA Crossover

## Strategy Specification

**Name:** EmaCrossover

**Description:** A trend-following strategy that uses two Exponential Moving Averages (EMA) with different lookback periods (Short and Long). It generates buy signals when the Short EMA crosses above the Long EMA, and sell signals when the Short EMA crosses below the Long EMA.

**Rationale:** EMAs react faster to price changes than SMAs. A crossover of a faster EMA above a slower EMA often signals the start of an uptrend.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `rust_decimal` for precise EMA calculation.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** Short EMA > Long EMA (and Short EMA was <= Long EMA in previous bar).

### Exit Conditions
- **Long Exit (Sell):** Short EMA < Long EMA.
- **Stop Loss:** Close Price <= Entry Price * (1 - `stop_loss_pct`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::ema;
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct EmaCrossover {
    config: EmaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EmaCrossoverConfig {
    pub short_window: usize,
    pub long_window: usize,
    pub stop_loss_pct: f64,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Explicitly checks if current price drops below a percentage of the entry price. This is crucial for limiting downside risk in false breakouts.
- **Statefulness:** The strategy tracks simulated position state during signal generation to correctly apply stop-loss logic based on the specific entry price.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Signal generation mimics real-time execution by iterating chronologically and updating state.

### Performance
- EMA calculation is O(N).
- Signal generation loop is O(N).
- Uses `Vec<Option<f64>>` for efficient indexed access during iteration.

---

# Trading Strategy: RSI Mean Reversion

## Strategy Specification

**Name:** RsiMeanReversion

**Description:** A mean reversion strategy that uses the Relative Strength Index (RSI) to identify overbought and oversold conditions. It buys when the RSI falls below a lower threshold (Oversold) and sells when the RSI rises above an upper threshold (Overbought).

**Rationale:** Extreme RSI values often indicate that an asset is overbought or oversold and a price reversal is likely.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `rust_decimal` for precise calculation.

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry (Buy):** RSI(period) < `oversold_threshold`

### Exit Conditions
- **Long Exit (Sell):** RSI(period) > `overbought_threshold`
- **Stop Loss:** Close Price <= Entry Price * (1 - `stop_loss_pct`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::rsi;
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct RsiMeanReversion {
    config: RsiMeanReversionConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RsiMeanReversionConfig {
    pub period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_pct: f64,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Includes a fixed percentage stop loss relative to the entry price.
- **Trend Filter:** Does not currently filter against the major trend (e.g. trading against a strong downtrend).

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires sufficient data length (at least `period` + 1) to calculate RSI.

### Performance
- RSI calculation is O(N).
- Signal generation is O(N).

---

# Trading Strategy: MACD (Moving Average Convergence Divergence)

## Strategy Specification

**Name:** Macd

**Description:** A trend-following momentum strategy that uses the Moving Average Convergence Divergence (MACD) indicator. It generates buy signals when the MACD line crosses above the Signal line, and sell signals when the MACD line crosses below the Signal line.

**Rationale:** MACD is one of the most popular and reliable indicators for identifying trend reversals and momentum. The crossover of the MACD line over the Signal line suggests a shift in momentum that often precedes a trend change.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `rust_decimal` for precise calculation.
- Calculates MACD(12, 26, 9) by default.

### Strategy Type
Trend Following / Momentum

### Entry Conditions
- **Long Entry (Buy):** MACD Line > Signal Line (and MACD Line was <= Signal Line in previous bar).

### Exit Conditions
- **Long Exit (Sell):** MACD Line < Signal Line.
- **Stop Loss:** Close Price <= Entry Price * (1 - `stop_loss_pct`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::macd;
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct Macd {
    config: MacdConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MacdConfig {
    pub fast_period: usize,
    pub slow_period: usize,
    pub signal_period: usize,
    pub stop_loss_pct: f64,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Explicitly checks if current price drops below a percentage of the entry price (default 5%).
- **Statefulness:** The strategy tracks simulated position state during signal generation to correctly apply stop-loss logic.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Signal generation mimics real-time execution by iterating chronologically and updating state.

### Performance
- MACD calculation is O(N).
- Signal generation loop is O(N).
- Uses `Vec<Option<f64>>` for efficient indexed access during iteration.

---

# Trading Strategy: Supertrend

## Strategy Specification

**Name:** Supertrend

**Description:** A trend-following strategy that uses the Average True Range (ATR) to define a trailing stop line (the Supertrend line). It generates buy signals when the price closes above the Supertrend line (indicating an uptrend) and sell signals when the price closes below the Supertrend line (indicating a downtrend).

**Rationale:** The Supertrend indicator is effective at identifying the primary trend and filtering out noise. It provides a clear trailing stop level, making risk management integral to the strategy.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `atr` indicator for volatility calculation.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** Close Price > Supertrend Line (and Trend changed from Down to Up).

### Exit Conditions
- **Long Exit (Sell):** Close Price < Supertrend Line (and Trend changed from Up to Down).
- **Stop Loss:** The Supertrend line itself acts as the dynamic stop loss.

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::atr;
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct Supertrend {
    config: SupertrendConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SupertrendConfig {
    pub period: usize,
    pub factor: f64,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** The Supertrend line is naturally a trailing stop loss. The strategy output explicitly sets the `stop_loss` field to the Supertrend value.
- **Statefulness:** The strategy tracks the previous trend and band values to determine flips and update the trailing stop.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Signal generation iterates through bars to simulate real-time state updates.

### Performance
- ATR calculation is O(N).
- Signal generation loop is O(N).
