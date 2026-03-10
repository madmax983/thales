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

# Trading Strategy: ADX + MACD Trend

## Strategy Specification

**Name:** AdxMacdTrend

**Description:** A trend-following strategy that combines the Average Directional Index (ADX) for trend strength and Moving Average Convergence Divergence (MACD) for trend direction. It enters long when the MACD Line crosses above the Signal Line, provided the ADX indicates a strong trend (ADX > `adx_threshold`).

**Rationale:** MACD is an excellent trend-following momentum indicator but is prone to false signals (whipsaws) in ranging markets. By filtering MACD crossovers with an ADX threshold (e.g., ADX > 25), we ensure that we only take MACD signals when a strong trend is established, significantly increasing the probability of a successful trade.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `adx`, `macd`, and `atr` indicators from the custom indicators module.

### Strategy Type
TrendFollowing

### Entry Conditions
- **Long Entry (Buy):** MACD Line crosses ABOVE Signal Line AND ADX > `adx_threshold`.

### Exit Conditions
- **Long Exit (Sell):** MACD Line crosses BELOW Signal Line OR Price hits Stop Loss.
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{adx, atr, macd};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct AdxMacdTrend {
    config: AdxMacdTrendConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AdxMacdTrendConfig {
    pub adx_period: usize,
    pub adx_threshold: f64,
    pub macd_fast_period: usize,
    pub macd_slow_period: usize,
    pub macd_signal_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for dynamic risk control.
- **Trend Filter:** ADX acts as a strict filter to avoid entering trades in sideways markets.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "high", "low", and "close".
- Requires data length > max(`macd_slow_period` + `macd_signal_period`, `adx_period`, `atr_period`).

### Performance
- MACD, ADX, and ATR calculations are O(N).
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
- **Expected Win Rate:** ~40-45% (Trend-following strategies typically have lower win rates but higher reward/risk ratios).
- **Expected Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 20%

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

---

# Trading Strategy: VWMA Crossover

## Strategy Specification

**Name:** VwmaCrossover

**Description:** A trend-following strategy that generates signals based on the crossover between a Volume Weighted Moving Average (VWMA) and a Simple Moving Average (SMA). It enters long when the VWMA crosses above the SMA, indicating that volume is supporting the upward price movement.

**Rationale:** Moving averages alone can lag and do not account for volume. By comparing a volume-weighted average (VWMA) with a standard moving average (SMA) of the same period, traders can identify situations where volume is confirming a price trend. When VWMA > SMA, it means higher volume occurred on up days, which is bullish.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `vwma`, `sma`, and `atr` indicators.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** VWMA crosses ABOVE SMA.
- **Short Entry (Sell):** VWMA crosses BELOW SMA.

### Exit Conditions
- **Long Exit (Sell):** VWMA crosses BELOW SMA.
- **Short Exit (Buy):** VWMA crosses ABOVE SMA.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, sma, vwma};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct VwmaCrossover {
    config: VwmaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VwmaCrossoverConfig {
    pub vwma_period: usize,
    pub sma_period: usize,
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
- Accepts `DataFrame` with historical data containing "close" and "volume".
- Requires data length > `max(vwma_period, sma_period)`.

### Performance
- VWMA calculation is O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: VWAP Reversion

## Strategy Specification

**Name:** VwapReversion

**Description:** A mean reversion strategy based on the Volume Weighted Average Price (VWAP). It buys when the price is significantly below the VWAP (oversold) and sells when the price is significantly above the VWAP (overbought).

**Rationale:** Prices tend to revert to their volume-weighted average over time. Extreme deviations from VWAP represent potential mean reversion opportunities as the market corrects overextensions.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `vwma` and `atr` indicators.

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry (Buy):** Close Price < Lower Band (VWMA * (1.0 - `oversold_threshold_pct`)).
- **Short Entry (Sell):** Close Price > Upper Band (VWMA * (1.0 + `overbought_threshold_pct`)).

### Exit Conditions
- **Long Exit (Sell):** Close Price >= VWMA.
- **Short Exit (Buy):** Close Price <= VWMA.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, vwma};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct VwapReversion {
    config: VwapReversionConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VwapReversionConfig {
    pub vwma_period: usize,
    pub oversold_threshold_pct: f64,
    pub overbought_threshold_pct: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to current market volatility.
- **Take Profit:** Mean reversion implies the target is the mean (VWMA).

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close" and "volume".
- Requires data length > `vwma_period`.

### Performance
- VWMA calculation is O(N).
- Signal generation loop is O(N).
---

# Trading Strategy: Vortex Breakout

## Strategy Specification

**Name:** VortexBreakout

**Description:** A trend-following strategy that generates signals based on the crossover of the positive and negative directional movement lines of the Vortex Indicator. It enters long when VI+ crosses above VI-, and enters short when VI+ crosses below VI-.

**Rationale:** The Vortex Indicator captures positive and negative trend movements to define trend direction and strength. A crossover between VI+ and VI- represents a shift in directional movement, signaling the potential start of a new trend or continuation.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `vortex` and `atr` indicators.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** VI+ crosses ABOVE VI-.
- **Short Entry (Sell):** VI+ crosses BELOW VI-.

### Exit Conditions
- **Long Exit (Sell):** VI+ crosses BELOW VI-.
- **Short Exit (Buy):** VI+ crosses ABOVE VI-.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, vortex};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct VortexBreakout {
    config: VortexBreakoutConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VortexBreakoutConfig {
    pub period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to volatility.
- **Take Profit:** The strategy implies trailing the trend until a reverse crossover occurs, but manual take profit logic could be added or dynamically set based on risk.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "high", "low", and "close".
- Requires data length > `period`.

### Performance
- Vortex Indicator calculation is O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: Z-Score Mean Reversion

## Strategy Specification

**Name:** ZScoreMeanReversion

**Description:** A mean reversion strategy based on the Z-Score. It calculates the Z-Score by measuring how many standard deviations the current price is away from a Simple Moving Average (SMA).

**Rationale:** Prices tend to revert to their moving average over time. High positive or negative Z-Scores indicate that the asset is overbought or oversold, providing potential mean reversion opportunities as the market corrects overextensions.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses custom `zscore` and `atr` indicators.

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry (Buy):** Z-Score crosses BELOW `-entry_threshold`.
- **Short Entry (Sell):** Z-Score crosses ABOVE `entry_threshold`.

### Exit Conditions
- **Long Exit (Sell):** Z-Score crosses ABOVE `exit_threshold`.
- **Short Exit (Buy):** Z-Score crosses BELOW `-exit_threshold`.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, zscore};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct ZScoreMeanReversion {
    config: ZScoreMeanReversionConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ZScoreMeanReversionConfig {
    pub period: usize,
    pub entry_threshold: f64,
    pub exit_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to current market volatility.
- **Take Profit:** Mean reversion implies the target is the mean (SMA).

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > `period`.

### Performance
- Z-Score calculation is O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: Chaikin Money Flow

## Strategy Specification

**Name:** ChaikinMoneyFlow

**Description:** A momentum strategy based on the Chaikin Money Flow (CMF) indicator. It measures buying and selling pressure by calculating the Money Flow Multiplier and Money Flow Volume over a given period (usually 21 periods).

**Rationale:** CMF combines price and volume to identify accumulation and distribution. A sustained positive reading indicates buying pressure, while a sustained negative reading indicates selling pressure. Crossovers of the zero line can signal a potential shift in trend momentum.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `cmf` and `atr` indicators.

### Strategy Type
Momentum / Trend Following

### Entry Conditions
- **Long Entry (Buy):** CMF crosses ABOVE `buy_threshold` (default 0.0).

### Exit Conditions
- **Long Exit (Sell):** CMF crosses BELOW `sell_threshold` (default 0.0).
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, cmf};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct ChaikinMoneyFlow {
    config: ChaikinMoneyFlowConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChaikinMoneyFlowConfig {
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
- **Take Profit:** The strategy implies trailing the momentum until a reverse crossover occurs.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "high", "low", "close", and "volume".
- Requires data length > `period`.

### Performance
- CMF calculation is O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: Elder Ray Index

## Strategy Specification

**Name:** ElderRay

**Description:** A trend-following strategy that uses the Elder Ray Index to measure buying and selling pressure in the market. It combines an Exponential Moving Average (EMA) with Bull Power and Bear Power indicators to generate trading signals.

**Rationale:** Developed by Dr. Alexander Elder, the Elder Ray Index helps traders identify when bulls or bears are in control of the market. The EMA indicates the consensus of value (the trend), while Bull Power and Bear Power measure the ability of buyers and sellers to drive prices above or below that consensus. Trades are taken in the direction of the trend when the opposing power shows weakness.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `elder_ray` and `atr` indicators.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** Bear Power < 0.0 AND Bear Power > Previous Bear Power (Bearish divergence / turning up) AND EMA > Previous EMA (Uptrend).
- **Short Entry (Sell):** Bull Power > 0.0 AND Bull Power < Previous Bull Power (Bullish divergence / turning down) AND EMA < Previous EMA (Downtrend).

### Exit Conditions
- **Long Exit (Sell):** Bear Power < Previous Bear Power OR EMA < Previous EMA (Trend weakening/reversing).
- **Short Exit (Buy):** Bull Power > Previous Bull Power OR EMA > Previous EMA (Trend weakening/reversing).
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, elder_ray};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct ElderRay {
    config: ElderRayConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ElderRayConfig {
    pub ema_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to volatility.
- **Trend Filter:** The EMA slope acts as a strict trend filter for entries.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "high", "low", and "close".
- Requires data length > `ema_period`.

### Performance
- Elder Ray calculation (EMA, High - EMA, Low - EMA) is O(N).
- Signal generation loop is O(N).

# Trading Strategy: Chandelier Exit

## Strategy Specification

**Name:** ChandelierExit

**Description:** A trend-following strategy that uses the Chandelier Exit indicator to trail a stop loss from the highest high or lowest low over a period, adjusted by the Average True Range (ATR).

**Rationale:** The Chandelier Exit helps traders ride a trend by allowing profits to run while cutting losses using a dynamic, volatility-adjusted stop level. When the price crosses above the long exit line or below the short exit line, a trend change is signaled.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses the custom `chandelier_exit` indicator.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** Close Price crosses ABOVE the Long Exit Line.
- **Short Entry (Sell):** Close Price crosses BELOW the Short Exit Line.

### Exit Conditions
- **Long Exit (Sell):** Close Price crosses BELOW the Long Exit Line.
- **Short Exit (Buy):** Close Price crosses ABOVE the Short Exit Line.
- **Stop Loss:** The Chandelier Exit line itself.

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::chandelier_exit;
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct ChandelierExit {
    config: ChandelierExitConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChandelierExitConfig {
    pub period: usize,
    pub atr_period: usize,
    pub multiplier: f64,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** The Chandelier Exit naturally provides a stop loss level.

### Backtesting Requirements
- Accepts `DataFrame` with historical data.
- Requires data length > `period`.
- **Expected Win Rate:** ~40-45% (Trend-following strategies typically have lower win rates but higher reward/risk ratios).
- **Expected Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 20%

### Performance
- Chandelier Exit calculation uses O(N) rolling max/min queue.
- Signal generation loop is O(N).

---

# Trading Strategy: Aroon Oscillator

## Strategy Specification

**Name:** AroonOscillator

**Description:** A trend-following strategy that uses the Aroon Oscillator to identify trend strength and direction. Aroon Up measures the number of periods since the highest high, and Aroon Down measures the number of periods since the lowest low within the period. The oscillator is the difference between Aroon Up and Aroon Down.

**Rationale:** The Aroon Oscillator is a powerful tool to detect strong trends early by measuring the time since recent highs and lows. When the oscillator crosses above a positive threshold (or zero), it indicates a strong emerging uptrend. A cross below a negative threshold (or zero) indicates a strong downtrend.

## Requirements

### Implementation Details
- Uses Polars for data analysis and signal generation.
- Implements Aroon indicator calculations manually.
- Evaluates crossovers based on consecutive closing candles.

### Strategy Type
TrendFollowing

### Entry Conditions
- **Long Entry (Buy):** Aroon Oscillator crosses above the `buy_threshold`.
- **Short Entry (Sell):** Aroon Oscillator crosses below the `sell_threshold`.

### Exit Conditions
- Uses Take Profit and Stop Loss derived from ATR.

### Position Sizing
- **Size Hint:** "100" units (fixed sizing placeholder).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use polars::prelude::*;
use rust_decimal::Decimal;
use async_trait::async_trait;
use anyhow::Result;

pub struct AroonOscillator {
    config: AroonOscillatorConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AroonOscillatorConfig {
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
- MUST include ATR-based stop-loss logic.
- Consider adjusting maximum position sizes during volatile markets.

### Backtesting Requirements
- The Aroon Oscillator logic is backtestable and verifies the exact `periods_since_high` and `periods_since_low` calculations using exact Rust iterations over the high/low arrays.

### Performance
- Calculates Aroon values in `O(N * M)` where N is length and M is period, fast enough for <100ms given typical periods (14-25).

# Trading Strategy: ROC Momentum

## Strategy Specification

**Name:** RocMomentum

**Description:** A momentum strategy based on the Rate of Change (ROC) indicator. It identifies overbought and oversold conditions, as well as trend reversals, by measuring the percentage change in price over a given period.

**Rationale:** Momentum often precedes price. A high positive ROC indicates strong bullish momentum (overbought), while a low negative ROC indicates strong bearish momentum (oversold). Crossing above/below specific thresholds can signal trend changes.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses the custom `roc` and `atr` indicators.

### Strategy Type
Momentum

### Entry Conditions
- **Long Entry (Buy):** ROC crosses ABOVE the `buy_threshold`.
- **Short Entry (Sell):** ROC crosses BELOW the `sell_threshold`.

### Exit Conditions
- Uses Take Profit and Stop Loss derived from ATR.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use polars::prelude::*;
use rust_decimal::Decimal;
use async_trait::async_trait;
use anyhow::Result;

pub struct RocMomentum {
    config: RocMomentumConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RocMomentumConfig {
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

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "high", "low", and "close".
- Requires data length > `period` and `atr_period`.
- Expected Win Rate: 45-55%
- Expected Sharpe Ratio: > 1.0
- Max Drawdown: < 15%

### Performance
- Calculates ROC values in O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: StochRSI Mean Reversion

## Strategy Specification

**Name:** StochRsiMeanReversion

**Description:** A mean reversion strategy based on the Stochastic RSI (StochRSI) indicator. It identifies overbought and oversold conditions by applying the stochastic formula to RSI values, providing high sensitivity to price extremes.

**Rationale:** Standard RSI can lag and stay in neutral zones for long periods. StochRSI measures where RSI is relative to its high-low range, making it a very sensitive momentum oscillator. When StochRSI reaches extreme values and its %K line crosses its %D line, it signals a potential mean reversion.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses the custom `stoch_rsi` indicator alongside `atr` for risk management.

### Strategy Type
MeanReversion

### Entry Conditions
- **Long Entry (Buy):** StochRSI %K crosses ABOVE StochRSI %D AND %K is LESS THAN `oversold_threshold` (e.g., 20).
- **Short Entry (Sell):** StochRSI %K crosses BELOW StochRSI %D AND %K is GREATER THAN `overbought_threshold` (e.g., 80).

### Exit Conditions
- Uses Take Profit (e.g., 2:1 Risk/Reward) and Stop Loss derived from ATR.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).
- Also signals exits on opposite crossovers in extreme regions.

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use polars::prelude::*;
use rust_decimal::Decimal;
use async_trait::async_trait;
use anyhow::Result;

pub struct StochRsiMeanReversion {
    config: StochRsiMeanReversionConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StochRsiMeanReversionConfig {
    pub rsi_period: usize,
    pub stoch_period: usize,
    pub k_period: usize,
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
- **Stop Loss:** Uses ATR-based stop loss to adapt to volatility and limit risk on premature entries.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > `rsi_period + stoch_period + k_period + d_period`.
- Expected Win Rate: 50-60% (High probability in ranging markets).
- Expected Sharpe Ratio: > 1.2
- Max Drawdown: < 15%

### Performance
- Calculates StochRSI values in O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: MACD + RSI Trend

## Strategy Specification

**Name:** MacdRsiTrend

**Description:** A trend-following momentum strategy combining the Moving Average Convergence Divergence (MACD) and the Relative Strength Index (RSI). It enters long when the MACD Line crosses above the Signal Line, provided the RSI is below a certain buy threshold (not overbought). It exits long when the MACD crosses below the Signal Line or the RSI exceeds a sell threshold.

**Rationale:** Using MACD alone can result in entering trades when the asset is already overbought and prone to reversal. Combining MACD with RSI ensures that momentum is shifting positively (MACD crossover) while there is still room for upward movement (RSI confirmation).

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `macd`, `rsi`, and `atr` indicators.

### Strategy Type
Momentum

### Entry Conditions
- **Long Entry (Buy):** MACD Line crosses ABOVE Signal Line AND RSI < `rsi_buy_threshold`.

### Exit Conditions
- **Long Exit (Sell):** MACD Line crosses BELOW Signal Line OR RSI > `rsi_sell_threshold`.
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, macd, rsi};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct MacdRsiTrend {
    config: MacdRsiTrendConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MacdRsiTrendConfig {
    pub macd_fast_period: usize,
    pub macd_slow_period: usize,
    pub macd_signal_period: usize,
    pub rsi_period: usize,
    pub rsi_buy_threshold: f64,
    pub rsi_sell_threshold: f64,
    pub atr_period: usize,
    pub stop_loss_atr_mult: f64,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for risk control.
- **Trend Filter:** RSI acts as a momentum confirmation and overbought filter.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > max(`macd_slow_period` + `macd_signal_period`, `rsi_period`, `atr_period`).

### Performance
- MACD, RSI, and ATR calculations are O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: TRIX Momentum

## Strategy Specification

**Name:** TrixMomentum

**Description:** A momentum strategy that uses the Triple Exponential Average (TRIX) indicator and its Signal Line (a Simple Moving Average of TRIX) to identify trend direction and momentum shifts. It enters long when the TRIX line crosses above its Signal Line and exits when it crosses below.

**Rationale:** The TRIX indicator acts as an oscillator that filters out insignificant price movements by using a triple exponentially smoothed moving average. When the TRIX line crosses above its signal line, it suggests that momentum is accelerating upwards, providing a strong entry signal for trend-following or momentum trading.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `trix`, `sma`, and `atr` indicators.

### Strategy Type
Momentum

### Entry Conditions
- **Long Entry (Buy):** TRIX line crosses ABOVE Signal Line (SMA of TRIX).

### Exit Conditions
- **Long Exit (Sell):** TRIX line crosses BELOW Signal Line.
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, sma, trix};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct TrixMomentum {
    config: TrixMomentumConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TrixMomentumConfig {
    pub trix_period: usize,
    pub signal_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for risk control, trailing dynamic volatility.
- **Trend Confirmation:** Relies on the TRIX crossover logic, which inherently smooths out false signals.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > `trix_period * 3 + signal_period + atr_period`.

### Performance
- TRIX, SMA, and ATR calculations are O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: TSI Trend

## Strategy Specification

**Name:** TsiTrend

**Description:** A momentum and trend-following strategy based on the True Strength Index (TSI). It enters long when the TSI line crosses above its Signal Line (an EMA of the TSI) and enters short when the TSI crosses below its Signal Line.

**Rationale:** The TSI measures trend direction and strength by double-smoothing momentum. The signal line crossover provides early warnings of trend changes while filtering out noise due to the double smoothing mechanism.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `tsi`, `ema`, and `atr` indicators.

### Strategy Type
Momentum

### Entry Conditions
- **Long Entry (Buy):** TSI line crosses ABOVE Signal Line (EMA of TSI).
- **Short Entry (Sell):** TSI line crosses BELOW Signal Line.

### Exit Conditions
- **Long Exit (Sell):** TSI line crosses BELOW Signal Line.
- **Short Exit (Buy):** TSI line crosses ABOVE Signal Line.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, ema, tsi};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct TsiTrend {
    config: TsiTrendConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TsiTrendConfig {
    pub long_period: usize,
    pub short_period: usize,
    pub signal_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to volatility.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > `long_period + short_period + signal_period + atr_period`.
- Expected Win Rate: 45-55%
- Expected Sharpe Ratio: > 1.0

### Performance
- TSI, EMA, and ATR calculations are O(N).
- Signal generation loop is O(N).

## StochRsiMeanReversion

**Name:** StochRsiMeanReversion
**Description:** Applies the Stochastic oscillator formula to the Relative Strength Index (RSI) to identify overbought and oversold conditions with greater sensitivity.
**Rationale:** Regular RSI can languish between 30 and 70 for extended periods. StochRSI is more sensitive and quickly identifies extremes in RSI itself.

### Signal Generation

- **Entry Long:** `%K` crosses above `%D` while both are below the `oversold_threshold` (default 20).
- **Entry Short:** `%K` crosses below `%D` while both are above the `overbought_threshold` (default 80).
- **Exit Long:** Price hits stop loss (ATR-based) OR `%K` crosses below `%D` above the `overbought_threshold`.
- **Exit Short:** Price hits stop loss (ATR-based) OR `%K` crosses above `%D` below the `oversold_threshold`.

---

# Trading Strategy: TEMA Crossover

## Strategy Specification

**Name:** TemaCrossover

**Description:** A trend-following strategy that generates signals based on the crossover of two Triple Exponential Moving Averages (TEMA) of different periods.

**Rationale:** The TEMA indicator reduces the lag associated with traditional moving averages while maintaining smoothing. Crossovers between a fast and slow TEMA provide timely entry and exit signals for trending markets.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `tema` and `atr` indicators.

### Strategy Type
TrendFollowing

### Entry Conditions
- **Long Entry (Buy):** Short TEMA crosses ABOVE Long TEMA.

### Exit Conditions
- **Long Exit (Sell):** Short TEMA crosses BELOW Long TEMA.
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, tema};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct TemaCrossover {
    config: TemaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TemaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for risk control.
- **Trend Filter:** Relying on TEMA reduces lag but can increase false signals in ranging markets.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > `long_period * 3`.

### Performance
- TEMA and ATR calculations are O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: DEMA Crossover

## Strategy Specification

**Name:** DemaCrossover

**Description:** A trend-following strategy that generates signals based on the crossover of two Double Exponential Moving Averages (DEMA) of different periods.

**Rationale:** The DEMA indicator reduces the lag associated with traditional moving averages while maintaining smoothing. Crossovers between a fast and slow DEMA provide timely entry and exit signals for trending markets with less delay compared to traditional SMA or EMA crossovers.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `dema` and `atr` indicators.

### Strategy Type
TrendFollowing

### Entry Conditions
- **Long Entry (Buy):** Short DEMA crosses ABOVE Long DEMA.

### Exit Conditions
- **Long Exit (Sell):** Short DEMA crosses BELOW Long DEMA.
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, dema};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct DemaCrossover {
    config: DemaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DemaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for risk control.
- **Trend Filter:** Relying on DEMA reduces lag but can increase false signals in ranging markets. Best used in strong trending conditions.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > `long_period * 2 + atr_period`.

### Performance
- DEMA and ATR calculations are O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: WMA Crossover

## Strategy Specification

**Name:** WmaCrossover

**Description:** A trend-following strategy that utilizes the crossover of two Weighted Moving Averages (WMA). It buys when a shorter-period WMA crosses above a longer-period WMA, and sells when the shorter-period WMA crosses below the longer-period WMA.

**Rationale:** The Weighted Moving Average assigns greater weight to more recent data points compared to a Simple Moving Average (SMA), allowing it to react more quickly to recent price changes. This makes the WMA Crossover strategy effective at capturing new trends early while filtering out some lag inherent in standard SMA crossovers.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Utilizes the `wma::calculate` indicator logic.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** Short WMA crosses above the Long WMA.

### Exit Conditions
- **Long Exit (Sell):** Short WMA crosses below the Long WMA or the dynamic ATR-based Stop Loss is triggered.

### Position Sizing
- **Size Hint:** "100" (fixed units) for entry, "max" for exits.
- Relies on risk management downstream to size appropriately.

---

# Trading Strategy: SupertrendEmaCrossover

## Strategy Specification

**Name:** SupertrendEmaCrossover

**Description:** A trend-following strategy that combines the Supertrend indicator and Exponential Moving Average (EMA) crossovers. It goes long when the Supertrend indicates an uptrend and the short-term EMA is above the long-term EMA, and goes short when the Supertrend indicates a downtrend and the short-term EMA is below the long-term EMA.

**Rationale:** Supertrend clearly identifies the main trend, reducing false signals. EMA crossovers improve sensitivity to trend changes. Combining the two provides additional confirmation for entries, thus increasing reliability.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses sliding window EMA calculation and ATR-based Supertrend.

### Strategy Type
Trend Following

### Entry Conditions
- **Long:** Supertrend indicates an uptrend (1) AND Short EMA crosses ABOVE Long EMA.
- **Short:** Supertrend indicates a downtrend (-1) AND Short EMA crosses BELOW Long EMA.

### Exit Conditions
- **Exit Long:** Short EMA crosses BELOW Long EMA.
- **Exit Short:** Short EMA crosses ABOVE Long EMA.

### Position Sizing
- Entries use a generic size hint of "100".
- Exits use a size hint of "max" to close the entire position.
- Includes dynamic stop-loss derived from the Average True Range (ATR).

### Backtesting Requirements
- **Expected Win Rate:** 45-55% (trend-following strategies typically have lower win rates but higher reward-to-risk ratios).
- **Sharpe Ratio:** Expected > 1.2 in trending markets.
- **Max Drawdown:** Expected to be controlled (<15%) due to ATR-based dynamic stop losses and Supertrend filtering.
