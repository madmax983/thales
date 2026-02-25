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

---

# Trading Strategy: Donchian Breakout

## Strategy Specification

**Name:** DonchianBreakout

**Description:** A trend-following strategy that generates signals based on breakouts of the Donchian Channels (rolling maximum and minimum). It buys when price breaks above the N-period high and sells when price breaks below the M-period low.

**Rationale:** Captures sustained trends by entering on new highs and exiting on new lows. The strategy assumes that new highs indicate a continuing uptrend.

## Requirements

### Implementation Details
- Uses `VecDeque` for efficient O(N) rolling maximum and minimum calculation.
- Implements the `Strategy` trait in Rust.
- Uses `atr` indicator for volatility-based stop loss sizing.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** Close Price > Upper Channel (rolling max of Highs over `entry_period`, shifted by 1).

### Exit Conditions
- **Long Exit (Sell):** Close Price < Lower Channel (rolling min of Lows over `exit_period`, shifted by 1).
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`). Fallback to Lower Channel or 5% if ATR unavailable.

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::atr;
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct DonchianBreakout {
    config: DonchianBreakoutConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DonchianBreakoutConfig {
    pub entry_period: usize,
    pub exit_period: usize,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for initial risk control.
- **Trailing Stop:** The Lower Channel acts as a natural trailing stop.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `max(entry_period, exit_period)`.

### Performance
- Rolling Max/Min calculation is O(N) using Monotonic Queue.
- Signal generation loop is O(N).

---

# Trading Strategy: Parabolic SAR

## Strategy Specification

**Name:** ParabolicSar

**Description:** A trend-following strategy that uses the Parabolic SAR (Stop and Reverse) indicator. It generates buy signals when the price crosses above the SAR (trend flips to Up) and sell signals when the price crosses below the SAR (trend flips to Down).

**Rationale:** The Parabolic SAR is designed to identify trend direction and reversals. It provides a trailing stop level that tightens as the trend accelerates, locking in profits.

## Requirements

### Implementation Details
- Uses iterative calculation (stateful) to compute SAR values.
- Implements the `Strategy` trait in Rust.
- Uses Polars for data handling but iterates over rows for SAR logic.

### Strategy Type
Trend Following / Trailing Stop

### Entry Conditions
- **Long Entry (Buy):** Price crosses above SAR (Trend flips from Down to Up).

### Exit Conditions
- **Long Exit (Sell):** Price crosses below SAR (Trend flips from Up to Down).
- **Stop Loss:** The SAR value itself acts as the dynamic trailing stop.

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::parabolic_sar;
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct ParabolicSar {
    config: ParabolicSarConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ParabolicSarConfig {
    pub start: f64,
    pub increment: f64,
    pub max: f64,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** The strategy naturally provides a stop loss level (the SAR value).
- **Trend Filter:** Best used in trending markets. Whipsaws in sideways markets are a known weakness.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires sufficient data length for the indicator to stabilize (initial SAR depends on start point).

### Performance
- SAR calculation is O(N) iterative loop.
- Efficient enough for typical timeframe data.

---

# Trading Strategy: Keltner Channel Breakout

## Strategy Specification

**Name:** KeltnerChannelBreakout

**Description:** A trend-following strategy that uses Keltner Channels (EMA +/- ATR bands) to identify breakouts. It generates buy signals when price closes above the Upper Channel and sell signals when price closes below the Lower Channel.

**Rationale:** Keltner Channels adapt to volatility using ATR. A breakout from the channel often signals the start of a new trend or the continuation of a strong move.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `ema` and `atr` indicators.

### Strategy Type
Trend Following / Breakout

### Entry Conditions
- **Long Entry (Buy):** Close Price > Upper Channel (EMA + Multiplier * ATR).
- **Short Entry (Sell):** Close Price < Lower Channel (EMA - Multiplier * ATR).

### Exit Conditions
- **Long Exit (Sell):** Close Price < Middle Band (EMA).
- **Short Exit (Buy):** Close Price > Middle Band (EMA).
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, ema};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct KeltnerChannelBreakout {
    config: KeltnerChannelBreakoutConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct KeltnerChannelBreakoutConfig {
    pub ema_period: usize,
    pub atr_period: usize,
    pub atr_multiplier: f64,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for initial risk control.
- **Trailing Stop:** The Middle Band (EMA) acts as a trend filter and exit point.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `max(ema_period, atr_period)`.

### Performance
- EMA and ATR calculations are O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: Stochastic Oscillator

## Strategy Specification

**Name:** StochasticOscillator

**Description:** A momentum strategy using the Stochastic Oscillator to identify overbought/oversold conditions and potential reversals. It generates buy signals when the %K line crosses above the %D line in oversold territory, and sell signals when %K crosses below %D in overbought territory.

**Rationale:** The Stochastic Oscillator compares the closing price to the price range over a specific period. It is effective in identifying turning points in a range-bound or trending market by signalling momentum shifts.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `stochastic` indicator (custom implementation of %K and %D).

### Strategy Type
Momentum / Mean Reversion

### Entry Conditions
- **Long Entry (Buy):** %K crosses ABOVE %D AND %K < `oversold_threshold`.

### Exit Conditions
- **Long Exit (Sell):** %K crosses BELOW %D AND %K > `overbought_threshold`.
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, stochastic};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct StochasticOscillator {
    config: StochasticOscillatorConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StochasticOscillatorConfig {
    pub k_period: usize,
    pub k_smoothing: usize,
    pub d_period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss.
- **Take Profit:** Sets a take profit at 2x the risk distance (2 * ATR).

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `k_period` + smoothing + `d_period`.

### Performance
- Stochastic calculation involves rolling Min/Max (O(N)).
- Signal generation loop is O(N).
