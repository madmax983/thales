# Trading Strategy: Bollinger Bands

## Strategy Specification

**Name:** BollingerBands

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

---

# Trading Strategy: ADX Momentum

## Strategy Specification

**Name:** AdxMomentum

**Description:** A trend-following strategy that uses the Average Directional Index (ADX) to determine trend strength and the Directional Movement Index (DMI) for direction. It enters when ADX indicates a strong trend and +DI crosses above -DI.

**Rationale:** Many trend-following strategies fail in ranging markets. ADX filters out weak trends, ensuring that trades are only taken when momentum is sufficient.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `adx` and `atr` indicators.

### Strategy Type
Trend Following / Momentum

### Entry Conditions
- **Long Entry (Buy):** ADX > `adx_threshold` (e.g., 25) AND +DI > -DI. (Condition became true in the current bar).

### Exit Conditions
- **Long Exit (Sell):** +DI < -DI (Trend Reversal) OR ADX < `adx_threshold` (Trend Weakening).
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{adx, atr};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct AdxMomentum {
    config: AdxMomentumConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AdxMomentumConfig {
    pub adx_period: usize,
    pub adx_threshold: f64,
    pub di_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for initial risk control.
- **Trend Filter:** ADX acts as a strong trend filter.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `adx_period` * 2 approx.

### Performance
- ADX calculation involves smoothing (O(N)).
- Signal generation loop is O(N).

---

# Trading Strategy: Ichimoku Cloud

## Strategy Specification

**Name:** IchimokuCloud

**Description:** A trend-following strategy that uses the Ichimoku Kinko Hyo system. It generates signals based on the Tenkan-sen / Kijun-sen crossover, filtered by the Price's position relative to the Cloud (Kumo).

**Rationale:** The Ichimoku Cloud provides a comprehensive view of support/resistance, trend direction, and momentum. The Tenkan-Kijun crossover is a classic signal, and the Cloud filter ensures trades are taken in the direction of the major trend.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `ichimoku` indicator for component calculation.

### Strategy Type
Trend Following / Momentum

### Entry Conditions
- **Long Entry (Buy):** Tenkan-sen crosses ABOVE Kijun-sen AND Price > Span A AND Price > Span B.
- **Short Entry (Sell):** Tenkan-sen crosses BELOW Kijun-sen AND Price < Span A AND Price < Span B.

### Exit Conditions
- **Stop Loss:** Kijun-sen (Dynamic Trailing Stop).
- **Take Profit:** Entry +/- 2 * Risk (where Risk = |Entry - Kijun-sen|).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::ichimoku;
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
    pub senkou_span_b_period: usize,
    pub senkou_span_offset: usize,
    pub chikou_span_offset: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses the Kijun-sen as a hard stop loss level.
- **Trend Filter:** The Cloud (Kumo) acts as a strict trend filter.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `senkou_span_b_period` + `senkou_span_offset` (approx 78 bars) for full Cloud validity.

### Performance
- Ichimoku calculation involves rolling Max/Min (O(N)).
- Signal generation loop is O(N).

---

# Trading Strategy: CCI Momentum

## Strategy Specification

**Name:** CciMomentum

**Description:** A momentum strategy using the Commodity Channel Index (CCI) to identify breakouts and strong trends. It enters long when CCI crosses above a buy threshold (e.g. 100) and exits when it crosses below a sell threshold (e.g. 0).

**Rationale:** CCI measures the current price level relative to an average price level over a given period of time. High positive values indicate that prices are unusually high relative to the average, which in momentum trading signals strong upside strength.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `cci` and `atr` indicators.
- Uses O(N) calculation for indicators.

### Strategy Type
Momentum / Breakout

### Entry Conditions
- **Long Entry (Buy):** CCI > `buy_threshold` (default 100) AND Prev CCI <= `buy_threshold`.

### Exit Conditions
- **Long Exit (Sell):** CCI < `sell_threshold` (default 0) AND Prev CCI >= `sell_threshold`.
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, cci};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct CciMomentum {
    config: CciMomentumConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CciMomentumConfig {
    pub period: usize,
    pub buy_threshold: f64,
    pub sell_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to volatility.
- **Trend Filter:** CCI > 100 itself acts as a strong momentum filter.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `period`.

### Performance
- CCI calculation is O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: Linear Regression Trend

## Strategy Specification

**Name:** LinearRegressionTrend

**Description:** A trend-following strategy that uses the slope of a Linear Regression line calculated on the logarithmic prices. It enters long when the slope is significantly positive and short when the slope is significantly negative.

**Rationale:** Linear Regression fits a straight line to the price data, providing a robust measure of the trend direction and strength. Using logarithmic prices makes the slope represent the exponential growth rate (percentage change), which is scale-invariant.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `linear_regression` indicator for slope calculation.
- Uses `atr` indicator for stop loss calculation.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** Slope > `slope_threshold`.
- **Short Entry (Sell):** Slope < -`slope_threshold`.

### Exit Conditions
- **Long Exit (Sell):** Slope < 0.0 (Trend Reversal) OR Price <= Stop Loss.
- **Short Exit (Buy):** Slope > 0.0 (Trend Reversal) OR Price >= Stop Loss.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, linear_regression};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct LinearRegressionTrend {
    config: LinearRegressionTrendConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LinearRegressionTrendConfig {
    pub period: usize,
    pub slope_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to volatility.
- **Trend Reversal:** Exits immediately if the trend slope flips direction.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `period`.

### Performance
- Linear Regression calculation is O(N) using optimized incremental updates.
- Signal generation loop is O(N).

---

# Trading Strategy: OBV Trend Following

## Strategy Specification

**Name:** ObvTrendFollowing

**Description:** A trend-following strategy that uses the On-Balance Volume (OBV) indicator. It generates signals when the OBV line crosses its Simple Moving Average (Signal Line). A crossover above the Signal Line indicates buying pressure (accumulation), while a crossover below indicates selling pressure (distribution).

**Rationale:** Volume often precedes price. Changes in the flow of volume (OBV) can signal a potential trend change before it becomes apparent in the price action alone.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `obv` indicator and `sma` (on OBV series) for signal generation.
- Uses `atr` for stop loss calculation.

### Strategy Type
Trend Following / Momentum

### Entry Conditions
- **Long Entry (Buy):** OBV crosses ABOVE OBV-SMA.
- **Short Entry (Sell):** OBV crosses BELOW OBV-SMA.

### Exit Conditions
- **Long Exit (Sell):** OBV crosses BELOW OBV-SMA.
- **Short Exit (Buy):** OBV crosses ABOVE OBV-SMA.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, obv, sma};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct ObvTrendFollowing {
    config: ObvTrendFollowingConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ObvTrendFollowingConfig {
    pub obv_sma_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to volatility.
- **Reversal:** The strategy assumes a "Stop and Reverse" approach if constantly in the market, but individual signals are generated for entry and exit.

### Backtesting Requirements
- Accepts `DataFrame` with historical data including "volume".
- Requires data length > `obv_sma_period`.

### Performance
- OBV calculation is O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: Money Flow Index

## Strategy Specification

**Name:** MoneyFlowIndex

**Description:** A mean reversion strategy that uses the Money Flow Index (MFI) to identify overbought and oversold conditions by combining price and volume. It buys when the MFI falls below a lower threshold (Oversold) and sells when the MFI rises above an upper threshold (Overbought).

**Rationale:** MFI is a volume-weighted RSI. It accounts for trading volume, providing a more robust measure of buying and selling pressure than price alone. Extreme values indicate potential reversals.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `mfi` indicator.

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry (Buy):** MFI < `oversold_threshold` (e.g., 20).

### Exit Conditions
- **Long Exit (Sell):** MFI > `overbought_threshold` (e.g., 80).
- **Stop Loss:** Close Price <= Entry Price * (1 - `stop_loss_pct`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::mfi;
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct MoneyFlowIndex {
    config: MoneyFlowIndexConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MoneyFlowIndexConfig {
    pub period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_pct: f64,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Fixed percentage stop loss.
- **Volume Confirmation:** MFI naturally integrates volume, acting as its own confirmation.

### Backtesting Requirements
- Accepts `DataFrame` with historical data including "volume".
- Requires data length > `period`.

### Performance
- MFI calculation is O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: Connors RSI Mean Reversion

## Strategy Specification

**Name:** ConnorsRsiMeanReversion

**Description:** A short-term mean reversion strategy using the Connors RSI (CRSI), which combines three components: 3-period RSI, 2-period Streak RSI, and 100-period Percent Rank of Returns. It identifies short-term pullbacks in trending markets.

**Rationale:** CRSI is more responsive than traditional RSI and uses streak duration and magnitude of moves to identify high-probability reversal points.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `connors_rsi` indicator (composite).

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry (Buy):** CRSI < `oversold_threshold` (default 10).

### Exit Conditions
- **Long Exit (Sell):** CRSI > `overbought_threshold` (default 90) OR Price > SMA(5) (if configured).
- **Stop Loss:** Close Price <= Entry Price * (1 - `stop_loss_pct`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{connors_rsi, sma};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct ConnorsRsiMeanReversion {
    config: ConnorsRsiMeanReversionConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConnorsRsiMeanReversionConfig {
    pub rsi_period: usize,
    pub streak_rsi_period: usize,
    pub rank_lookback: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_pct: f64,
    pub exit_sma_period: Option<usize>,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses fixed percentage stop loss, but typically exits on short-term mean reversion (SMA crossing).

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `rank_lookback` (default 100).

### Performance
- CRSI calculation involves Percent Rank (O(N) window).
- Signal generation loop is O(N).

---

# Trading Strategy: Awesome Oscillator

## Strategy Specification

**Name:** AwesomeOscillator

**Description:** A momentum strategy using the Awesome Oscillator (AO) to identify momentum shifts. It enters long when the AO crosses above the zero line and enters short when the AO crosses below the zero line.

**Rationale:** The Awesome Oscillator measures market momentum by comparing a 5-period SMA of the median price with a 34-period SMA of the median price. A crossover of the zero line indicates a shift in momentum direction.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `awesome_oscillator` and `atr` indicators.

### Strategy Type
Momentum

### Entry Conditions
- **Long Entry (Buy):** AO crosses ABOVE 0.0.
- **Short Entry (Sell):** AO crosses BELOW 0.0.

### Exit Conditions
- **Long Exit (Sell):** AO crosses BELOW 0.0.
- **Short Exit (Buy):** AO crosses ABOVE 0.0.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, awesome_oscillator};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct AwesomeOscillator {
    config: AwesomeOscillatorConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AwesomeOscillatorConfig {
    pub fast_period: usize,
    pub slow_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to volatility.
- **Take Profit:** Sets a take profit at 2x the risk distance (2 * ATR).

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `slow_period`.

### Performance
- AO calculation is O(N).
- Signal generation loop is O(N).
---

# Trading Strategy: Chandelier Exit

## Strategy Specification

**Name:** ChandelierExit

**Description:** A trend-following strategy using the Chandelier Exit indicator to trail stops and identify trend changes. It enters long when the price crosses above the short Chandelier Exit line and enters short when the price crosses below the long Chandelier Exit line.

**Rationale:** Chandelier Exit trails stops using the ATR to adapt to market volatility, providing a natural buffer against whipsaws while keeping traders in major trends.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `atr` indicator.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** Close crosses ABOVE Chandelier Exit Short.
- **Short Entry (Sell):** Close crosses BELOW Chandelier Exit Long.

### Exit Conditions
- **Long Exit (Sell):** Close crosses BELOW Chandelier Exit Long.
- **Short Exit (Buy):** Close crosses ABOVE Chandelier Exit Short.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::atr;
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct ChandelierExit {
    config: ChandelierExitConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChandelierExitConfig {
    pub period: usize,
    pub atr_mult: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to volatility.
- **Take Profit:** Sets a take profit at 2x the risk distance (2 * ATR).

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `period`.

### Performance
- Indicator calculation is O(N).
- Signal generation loop is O(N).
---

# Trading Strategy: Williams %R

## Strategy Specification

**Name:** WilliamsR

**Description:** A momentum indicator that measures overbought and oversold levels, similar to the Stochastic Oscillator. It ranges from -100 to 0.

**Rationale:** It helps identify entry and exit points by showing when an asset is overbought (close to 0) or oversold (close to -100).

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `williams_r` and `atr` indicators.

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry (Buy):** Williams %R crosses ABOVE `oversold_threshold` (default -80.0).
- **Short Entry (Sell):** Williams %R crosses BELOW `overbought_threshold` (default -20.0).

### Exit Conditions
- **Long Exit (Sell):** Williams %R crosses BELOW `overbought_threshold`.
- **Short Exit (Buy):** Williams %R crosses ABOVE `oversold_threshold`.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, williams_r};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct WilliamsR {
    config: WilliamsRConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WilliamsRConfig {
    pub period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to volatility.
- **Take Profit:** Sets a take profit at 2x the risk distance (2 * ATR).

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `period`.

### Performance
- Williams %R calculation is O(N).
- Signal generation loop is O(N).
