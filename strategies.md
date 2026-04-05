# Trading Strategy: ZLEMA Crossover

## Strategy Specification

**Name:** ZlemaCrossover

**Description:** A trend following strategy that uses two Zero Lag Exponential Moving Averages (ZLEMA). It enters a long position when the faster ZLEMA crosses above the slower ZLEMA, and exits when it crosses below.

**Rationale:** The Zero Lag Exponential Moving Average is designed to eliminate the lag inherently present in standard moving averages. By identifying crossovers earlier than a standard EMA or SMA crossover strategy, it attempts to capture trend continuations and reversals with less delay.

## Requirements

### Implementation Details
- Uses Polars for data analysis and generating signals.
- Implements the `Strategy` trait in Rust.
- Utilizes the `zlema` and `atr` indicators.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** Short ZLEMA crosses above Long ZLEMA.

### Exit Conditions
- **Long Exit (Sell):** Short ZLEMA crosses below Long ZLEMA.

### Expected Backtesting Metrics
- **Win Rate:** ~45-55%
- **Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%

---

# Trading Strategy: CMO Mean Reversion

## Strategy Specification

**Name:** CmoMeanReversion

**Description:** A mean reversion strategy based on the Chande Momentum Oscillator (CMO). It buys when the asset is oversold (CMO crosses above a negative threshold) and sells when overbought (CMO crosses below a positive threshold).

**Rationale:** The Chande Momentum Oscillator directly measures momentum on a -100 to +100 scale by dividing the difference of gains and losses by the sum of total price movements. It identifies clear overbought and oversold conditions with high accuracy, establishing high win rate opportunities when momentum reverts.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Calculates ATR for dynamic stop losses.

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry (Buy):** CMO crosses above `-oversold_threshold` (e.g. -50).

### Exit Conditions
- **Long Exit (Sell):** CMO crosses below `overbought_threshold` (e.g. +50).

### Position Sizing
- **Size Hint:** "100" for entry, "max" for exit.

---

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

---

# Trading Strategy: HMA Crossover

## Strategy Specification

**Name:** HmaCrossover

**Description:** A trend-following strategy that generates signals based on the crossover of two Hull Moving Averages (HMA) of different periods.

**Rationale:** The HMA indicator aims to eliminate lag altogether while improving smoothing compared to traditional moving averages. Crossovers between a fast and slow HMA provide timely entry and exit signals for trending markets without the typical delay of SMAs or EMAs.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `hma` and `atr` indicators.

### Strategy Type
TrendFollowing

### Entry Conditions
- **Long Entry (Buy):** Short HMA crosses ABOVE Long HMA.

### Exit Conditions
- **Long Exit (Sell):** Short HMA crosses BELOW Long HMA.
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for risk control.
- **Trend Filter:** Relying on HMA eliminates lag but can increase false signals in ranging markets. Best used in strong trending conditions.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > `long_period * 2`.
- **Expected Win Rate:** 45-55%
- **Expected Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%

---

# Trading Strategy: Volume-Weighted MACD

## Strategy Specification

**Name:** VwMacd

**Description:** A variation of MACD that weights price by volume, aiming to filter out low-volume false signals. It uses VWMA (Volume Weighted Moving Average) instead of EMA for the fast and slow lines.

**Rationale:** Standard MACD relies on EMAs which only account for price. By incorporating volume through VWMA, VW-MACD ensures that momentum signals are backed by significant trading activity, potentially reducing false breakouts in low-volume environments.

## Requirements

### Implementation Details
- Uses Polars for data analysis and signal generation.
- Implements the `Strategy` trait in Rust.
- Uses `vwma`, `ema`, and `atr` indicators.

### Strategy Type
Momentum

### Entry Conditions
- **Short Entry (Sell):** VW-MACD crosses BELOW Signal line while VW-MACD > 0.
- **Long Entry (Buy):** VW-MACD crosses ABOVE Signal line while VW-MACD < 0.

### Exit Conditions
- **Long Exit (Sell):** VW-MACD crosses BELOW Signal line.
- **Short Exit (Buy):** VW-MACD crosses ABOVE Signal line.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** Bounded by `max_position_size`.

### Expected Backtesting Metrics
- **Expected Win Rate:** ~45-55%
- **Expected Sharpe Ratio:** > 1.0
- **Max Drawdown:** < 20%

## KdjIndicatorStrategy

### Strategy Specification
- **Name:** KDJ Indicator Trading Strategy
- **Description:** A mean-reversion and momentum strategy based on the KDJ indicator. It relies on the fast %K line, slow %D line, and divergence %J line to identify overbought/oversold conditions and trend reversals.
- **Rationale:** KDJ extends the Stochastic Oscillator by adding the J line, which represents the divergence of %K from %D. The J line is highly sensitive to price momentum, often crossing above/below 0 or 100 before actual price reversals occur, making it a strong leading indicator.

### Requirements
- Polars implementation.
- Backtestable logic.
- Returns explicit signals (buy/sell).
- Integrates ATR-based Stop Loss.

### Strategy Type
MeanReversion

### Entry Conditions
- **Long Entry:** %J line crosses above 0 (oversold reversal) OR %K crosses above %D while both are below 20.
- **Short Entry:** %J line crosses below 100 (overbought reversal) OR %K crosses below %D while both are above 80.

### Exit Conditions
- **Long Exit:** %J line crosses above 100 OR %K crosses below %D.
- **Short Exit:** %J line crosses below 0 OR %K crosses above %D.
- **Stop Loss:** Calculated via ATR distance (e.g. `price - ATR * stop_loss_atr_mult`).

### Position Sizing
- Fixed allocation bounded by `max_position_size` per trade.

### Expected Backtesting Metrics
- **Expected Win Rate:** 50-60%
- **Expected Sharpe Ratio:** > 1.0
- **Max Drawdown:** < 15%

---

# Trading Strategy: TTM Squeeze

## Strategy Specification

**Name:** TtmSqueeze

**Description:** A volatility and momentum strategy that capitalizes on periods of low volatility (the "squeeze") followed by a breakout, identified when Bollinger Bands move outside of Keltner Channels. The direction of the trade is determined by momentum.

**Rationale:** Markets alternate between periods of high and low volatility. By identifying a period of extreme low volatility (a squeeze), traders can anticipate a significant price movement. Momentum confirms the direction.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `bollinger_bands`, `keltner_channels`, `sma`, and `atr` indicators.

### Strategy Type
Momentum

### Entry Conditions
- **Long Entry (Buy):** Squeeze turns OFF (from ON) AND Momentum > 0.
- **Short Entry (Sell):** Squeeze turns OFF (from ON) AND Momentum < 0.

### Exit Conditions
- **Long Exit (Sell):** Momentum decreases (Momentum < previous Momentum) OR Stop Loss is hit.
- **Short Exit (Buy):** Momentum increases (Momentum > previous Momentum) OR Stop Loss is hit.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) for entry, "max" for exits.

### Expected Backtesting Metrics
- **Expected Win Rate:** ~45-55%
- **Expected Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%

---

# Trading Strategy: Disparity Index Reversion

## Strategy Specification

**Name:** DisparityIndexReversion

**Description:** A mean reversion strategy based on the Disparity Index, which measures the relative position of the latest closing price to a chosen moving average.

**Rationale:** When the price deviates significantly from its moving average (high disparity), it is likely to revert to the mean. Extreme negative values suggest oversold conditions, while extreme positive values suggest overbought conditions.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses the `disparity_index` and `atr` indicators.

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry (Buy):** Disparity Index drops below the `oversold_threshold` (e.g., -5%).
- **Short Entry (Sell):** Disparity Index rises above the `overbought_threshold` (e.g., +5%).

### Exit Conditions
- **Long Exit (Sell):** Disparity Index rises above 0 (returns to mean).
- **Short Exit (Buy):** Disparity Index drops below 0 (returns to mean).
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" for entry, "max" for exits.

### Expected Backtesting Metrics
- **Expected Win Rate:** 50% - 60% (typical for mean reversion)
- **Sharpe Ratio:** > 1.0
- **Max Drawdown:** < 15%

### Performance
- HMA and ATR calculations are O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: AlmaCrossover

## Strategy Specification

**Name:** AlmaCrossover

**Description:** A trend-following strategy that generates signals based on the crossover of two Arnaud Legoux Moving Averages (ALMA) of different periods.

**Rationale:** The ALMA indicator aims to reduce lag while increasing smoothness compared to traditional moving averages. Crossovers between a fast and slow ALMA provide timely entry and exit signals for trending markets.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `alma` and `atr` indicators.

### Strategy Type
TrendFollowing

### Entry Conditions
- **Long Entry (Buy):** Fast ALMA crosses ABOVE Slow ALMA.

### Exit Conditions
- **Long Exit (Sell):** Fast ALMA crosses BELOW Slow ALMA.
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, alma};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct AlmaCrossover {
    config: AlmaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AlmaCrossoverConfig {
    pub fast_period: usize,
    pub slow_period: usize,
    pub offset: f64,
    pub sigma: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for risk control.
- **Trend Filter:** Relying on ALMA reduces lag but can increase false signals in ranging markets. Best used in strong trending conditions.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > `slow_period`.

### Performance
- ALMA and ATR calculations are O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: MACD Trend Follower

## Strategy Specification

**Name:** MacdTrendFollower

**Description:** A trend-following strategy based on the Moving Average Convergence Divergence (MACD) indicator. It enters long when the MACD line crosses above the Signal line, and enters short when the MACD line crosses below the Signal line.

**Rationale:** The MACD is a powerful trend-following momentum indicator. Crossovers between the MACD line and its Signal line help identify shifts in momentum and the start of new trends. This strategy captures those trends and manages risk dynamically using an ATR-based stop loss.

## Requirements

### Implementation Details
- Uses Polars for data analysis and signal generation.
- Implements the `Strategy` trait in Rust.
- Utilizes custom `macd` and `atr` indicators.
- Validates parameters to ensure correct ranges (e.g., `fast_period < slow_period`).

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** MACD Line crosses ABOVE Signal Line.
- **Short Entry (Sell):** MACD Line crosses BELOW Signal Line.

### Exit Conditions
- **Long Exit (Sell):** MACD Line crosses BELOW Signal Line or Stop Loss is hit.
- **Short Exit (Buy):** MACD Line crosses ABOVE Signal Line or Stop Loss is hit.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

### Expected Backtesting Metrics
- **Expected Win Rate:** 45% - 50%
- **Expected Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 20%

---

# Trading Strategy: KAMA Crossover

## Strategy Specification

**Name:** KamaCrossover

**Description:** A trend-following strategy that generates signals based on the crossover of two Kaufman's Adaptive Moving Averages (KAMA) of different periods.

**Rationale:** The KAMA Crossover strategy is a trend-following approach that adapts to market noise. By using KAMA, the moving average closely follows prices when price swings are relatively small and noise is low, and adjusts to moving averages when prices swing widely and noise is high. A fast KAMA crossing above a slow KAMA suggests an emerging uptrend (bullish), while a fast KAMA crossing below a slow KAMA suggests an emerging downtrend (bearish).

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `kama` and `atr` indicators.

### Strategy Type
TrendFollowing

### Entry Conditions
- **Long Entry (Buy):** Short KAMA crosses ABOVE Long KAMA.
- **Short Entry (Sell):** Short KAMA crosses BELOW Long KAMA.

### Exit Conditions
- **Long Exit (Sell):** Short KAMA crosses BELOW Long KAMA.
- **Short Exit (Buy):** Short KAMA crosses ABOVE Long KAMA.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, kama};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct KamaCrossover {
    config: KamaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct KamaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub short_fast_ema_period: usize,
    pub short_slow_ema_period: usize,
    pub long_fast_ema_period: usize,
    pub long_slow_ema_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for dynamic risk control.
- **Trend Filter:** Adaptive nature of KAMA reduces false signals in ranging markets compared to SMA/EMA crossovers.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > `long_period`.
- **Expected Win Rate:** 45-55%
- **Expected Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%

### Performance
- KAMA and ATR calculations are O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: SMA Crossover

## Strategy Specification

**Name:** SmaCrossover

**Description:** A trend-following strategy that generates signals based on the crossover of two Simple Moving Averages (SMA) of different periods.

**Rationale:** The SMA Crossover strategy is a classic trend-following approach. It reduces market noise by averaging past prices, allowing traders to identify the general direction of the trend. A fast SMA crossing above a slow SMA suggests an emerging uptrend (bullish), while a fast SMA crossing below a slow SMA suggests an emerging downtrend (bearish).

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `sma` and `atr` indicators.

### Strategy Type
TrendFollowing

### Entry Conditions
- **Long Entry (Buy):** Short SMA crosses ABOVE Long SMA.
- **Short Entry (Sell):** Short SMA crosses BELOW Long SMA.

### Exit Conditions
- **Long Exit (Sell):** Short SMA crosses BELOW Long SMA.
- **Short Exit (Buy):** Short SMA crosses ABOVE Long SMA.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, sma};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct SmaCrossover {
    config: SmaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SmaCrossoverConfig {
    pub short_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for dynamic risk control.
- **Trend Filter:** Relying on SMA can increase lag and produce false signals in ranging markets. Best used in strong trending conditions or combined with other filters.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > `long_period`.
- **Expected Win Rate:** 40-50%
- **Expected Sharpe Ratio:** > 1.0
- **Max Drawdown:** < 20%

### Performance
- SMA and ATR calculations are O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: Bollinger RSI Mean Reversion

## Strategy Specification

**Name:** BollingerRsiMeanReversion

**Description:** A mean-reversion strategy combining Bollinger Bands and the Relative Strength Index (RSI).

**Rationale:** The strategy capitalizes on overextended price movements. It assumes that when price crosses below the lower Bollinger Band while RSI is oversold, the asset is undervalued and due for a bounce. Conversely, when price crosses above the upper Bollinger Band while RSI is overbought, the asset is overvalued and likely to retrace. Combining both indicators reduces false signals.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `bollinger_bands`, `rsi`, and `atr` indicators.

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry (Buy):** Price closes below the Lower Bollinger Band AND RSI is less than the oversold threshold.

### Exit Conditions
- **Long Exit (Sell):** Price closes above the Upper Bollinger Band OR RSI is greater than the overbought threshold OR Stop Loss is hit.
- **Stop Loss:** Entry Price - (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) for entry, "max" for exits.

---

# Trading Strategy: EmaRsiTrendFollowing

## Strategy Specification

**Name:** EmaRsiTrendFollowing

**Description:** A quantitative trading strategy that uses Exponential Moving Averages (EMA) and the Relative Strength Index (RSI) to capture short-term momentum and identify trend reversals.

**Rationale:** The strategy combines the trend-following capabilities of EMAs with the momentum confirmation of RSI. A short-term EMA crossing above a long-term EMA suggests an emerging uptrend, while RSI confirming momentum strength (e.g., RSI > 50) provides additional confidence for long entries. Conversely, a bearish EMA crossover with RSI < 50 signals a downtrend. Using both indicators reduces false signals by ensuring conflux between trend direction and momentum.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `ema` and `rsi` indicators.

### Strategy Type
TrendFollowing

### Entry Conditions
- **Long Entry (Buy):** Short EMA crosses ABOVE Long EMA and RSI > Buy Threshold.
- **Short Entry (Sell):** Short EMA crosses BELOW Long EMA and RSI < Sell Threshold.

### Exit Conditions
- **Long Exit (Sell):** Short EMA crosses BELOW Long EMA and RSI < Sell Threshold.
- **Short Exit (Buy):** Short EMA crosses ABOVE Long EMA and RSI > Buy Threshold.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, ema, rsi};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct EmaRsiTrendFollowing {
    config: EmaRsiTrendFollowingConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EmaRsiTrendFollowingConfig {
    pub short_ema_period: usize,
    pub long_ema_period: usize,
    pub rsi_period: usize,
    pub rsi_buy_threshold: f64,
    pub rsi_sell_threshold: f64,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for dynamic risk control.
- **Trend Filter:** Combining EMA crossovers with RSI momentum filters helps eliminate false positive entries during ranging or weak trend markets.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > max(`long_ema_period`, `rsi_period`).
- **Expected Win Rate:** 45-55%
- **Expected Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%

### Performance
- EMA, RSI, and ATR calculations are O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: Supertrend RSI

## Strategy Specification

**Name:** SupertrendRsi

**Description:** A mean-reversion and trend-following hybrid strategy combining Supertrend and the Relative Strength Index (RSI).

**Rationale:** The strategy capitalizes on the trend direction provided by Supertrend, while using RSI to find optimal entry points during pullbacks. It assumes that when the trend is up (Supertrend) and the asset is oversold (RSI), it is a good buying opportunity. Exit relies on the Supertrend flipping direction.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `supertrend`, `rsi`, and `atr` indicators.

### Strategy Type
MeanReversion

### Entry Conditions
- **Long Entry (Buy):** Price closes above Supertrend AND RSI < Oversold.
- **Short Entry (Sell):** Price closes below Supertrend AND RSI > Overbought.

### Exit Conditions
- **Long Exit (Sell):** Supertrend flips to Bearish.
- **Short Exit (Buy):** Supertrend flips to Bullish.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) for entry, "max" for exits.

---

# Trading Strategy: Triple SMA Crossover

## Strategy Specification

**Name:** TripleSmaCrossover

**Description:** A trend-following strategy that generates signals based on the crossover of three Simple Moving Averages (SMA): Short, Medium, and Long.

**Rationale:** Using three moving averages reduces false signals compared to a dual moving average crossover. A bullish signal is only generated when all three moving averages align in an upward direction (Short > Medium > Long), and a bearish signal when they align in a downward direction. Exits occur when the short-term moving average crosses the medium-term moving average.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `sma` and `atr` indicators.

### Strategy Type
TrendFollowing

### Entry Conditions
- **Long Entry (Buy):** Short SMA > Medium SMA > Long SMA (and not previously true).
- **Short Entry (Sell):** Short SMA < Medium SMA < Long SMA (and not previously true).

### Exit Conditions
- **Long Exit (Sell):** Short SMA < Medium SMA.
- **Short Exit (Buy):** Short SMA > Medium SMA.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, sma};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct TripleSmaCrossover {
    config: TripleSmaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TripleSmaCrossoverConfig {
    pub short_period: usize,
    pub medium_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for dynamic risk control.
- **Trend Filter:** Requiring three moving averages to align acts as a stronger trend filter, delaying entries but increasing confidence.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > `long_period`.
- **Expected Win Rate:** 40-50%
- **Expected Sharpe Ratio:** > 1.0
- **Max Drawdown:** < 15%

### Performance
- SMA and ATR calculations are O(N).
- Signal generation loop is O(N).

# Trading Strategy: Volume Oscillator Trend

## Strategy Specification

**Name:** VolumeOscillatorTrend

**Description:** A trend following strategy that uses the Volume Oscillator to confirm volume expansion alongside price trends defined by a Simple Moving Average. It enters long when the Volume Oscillator crosses above zero while price is above its SMA, and short when the VO crosses zero while price is below SMA.

**Rationale:** Expanding volume (VO > 0) often precedes or confirms significant price moves. Combining volume expansion with a price trend filter ensures trades are taken in the direction of the dominant momentum backed by participation.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Calculates ATR for dynamic stop losses.

### Strategy Type
Trend Following

# Trading Strategy: VPT Trend Following

## Strategy Specification

**Name:** VptTrendFollowing

**Description:** A trend-following strategy based on the Volume Price Trend (VPT) indicator.

**Rationale:** The strategy combines the momentum and volume features of the VPT indicator with price trends. When the VPT crosses above its SMA while the price is above its SMA, it signals a strong upward trend confirmed by volume. Conversely, a cross below signals a downward trend.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Calculates ATR for dynamic stop losses.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** VPT crosses above its SMA and Price is greater than the Price SMA.
- **Short Entry (Sell):** VPT crosses below its SMA and Price is less than the Price SMA.

### Exit Conditions
- **Long Exit:** VPT crosses below its SMA.
- **Short Exit:** VPT crosses above its SMA.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) for entry, "max" for exits.

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, sma, vpt};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct VptTrend {
    config: VptTrendConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VptTrendConfig {
    pub vpt_sma_period: usize,
    pub price_sma_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for dynamic risk control.
- **Max Position Size:** Size hint controls entry exposure.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close", "high", "low", and "volume".
- Required parameters: `vpt_sma_period`, `price_sma_period`, `atr_period`.

### Performance
- VPT, SMA, and ATR calculations are optimized.

## Strategy Specification

**Name:** PpoRsiTrend

**Description:** A trend-following strategy that combines the Percentage Price Oscillator (PPO) and the Relative Strength Index (RSI). It enters long when the PPO line crosses above its Signal line and the RSI is not overbought (below the sell threshold). It exits long when the PPO crosses below its Signal line or RSI becomes overbought. The inverse logic applies to short positions.

**Rationale:** The Percentage Price Oscillator (PPO) measures momentum based on the percentage difference between two moving averages, allowing for better comparability across different timeframes and asset prices than MACD. Coupling PPO with RSI ensures that trades are only taken when momentum is shifting favorably and the asset is not already overextended, reducing the likelihood of entering trades right before a reversal.

### Historical Performance & Backtesting Metrics
- **Expected Win Rate:** 45-55%
- **Expected Sharpe Ratio:** 1.2 - 1.5
- **Expected Max Drawdown:** 15-20%

# Trading Strategy: Force Index Trend

## Strategy Specification

**Name:** ForceIndexTrend

**Description:** A trend-following strategy using Alexander Elder's Force Index. It generates trading signals when the Force Index (smoothed with an EMA) crosses the zero line, while confirming the overall trend direction using a longer-term Price EMA.

**Rationale:** The Force Index combines price movement and volume to measure the power behind market moves. By smoothing it with a short-term moving average (e.g., 13 periods) and using a longer-term moving average (e.g., 22 periods) to establish the primary trend, the strategy seeks to capture momentum bursts that align with the broader market direction.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Calculates Force Index, smooths it with an EMA, and compares price against a Price EMA.
- Calculates ATR for dynamic stop losses.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry:** The closing price is above the Price EMA (Uptrend) AND the smoothed Force Index crosses from below zero to above zero.
- **Short Entry:** The closing price is below the Price EMA (Downtrend) AND the smoothed Force Index crosses from above zero to below zero.

### Exit Conditions
- Exits active positions when an opposite entry signal occurs.

### Position Sizing
- Fixed 100 units base per trade (dynamic sizing allowed depending on risk context).
- Stop Loss placed at entry price minus `stop_loss_atr_mult` * ATR for Long positions (plus for Short).
- Take profit placed at a 2:1 Reward to Risk ratio.

---

# Trading Strategy: Choppiness Index Trend

## Strategy Specification

**Name:** ChoppinessIndexTrend

**Description:** A trend-following strategy that generates signals based on the Choppiness Index (CHOP) and a Simple Moving Average (SMA).

**Rationale:** The Choppiness Index determines whether the market is choppy (trading sideways) or trending. When the market transitions from choppy to trending (CHOP drops below a threshold), the strategy uses a Simple Moving Average to determine the direction of the trend and enters a position.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `choppiness_index`, `sma`, and `atr` indicators.

### Strategy Type
TrendFollowing

### Entry Conditions
- **Long Entry (Buy):** CHOP drops below the threshold (e.g., 61.8) AND Price > SMA.
- **Short Entry (Sell):** CHOP drops below the threshold AND Price < SMA.

### Exit Conditions
- **Long/Short Exit:** CHOP rises above the threshold (indicating a return to choppiness).
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) for entry, "max" for exits.

### Historical Performance (Sample Metrics)
- **Expected Win Rate:** 55% - 62%
- **Sharpe Ratio:** 1.4
- **Max Drawdown:** 12%

## TrixCrossover
**Description:** Momentum trend-following strategy based on the TRIX indicator and its SMA signal line.
**Rationale:** Captures major market trends by identifying when momentum shifts directions. The TRIX indicator effectively filters out market noise.

### Requirements
- **Strategy Type:** Momentum
- **Implementation:** Rust (`TrixCrossover`)

### Entry Conditions
- **Long:** TRIX crosses ABOVE its Signal line, and TRIX is below zero.
- **Short:** TRIX crosses BELOW its Signal line, and TRIX is above zero.

### Exit Conditions
- **Long:** TRIX crosses BELOW its Signal line.
- **Short:** TRIX crosses ABOVE its Signal line.

### Position Sizing
- Fixed 100 units base per trade (dynamic sizing allowed depending on risk context).
- Stop Loss placed at entry price minus `stop_loss_atr_mult` * ATR for Long positions (plus for Short).
- Take profit placed at a 2:1 Reward to Risk ratio.
---

# Trading Strategy: Chaikin Oscillator Momentum

## Strategy Specification

**Name:** ChaikinOscillatorMomentum

**Description:** A momentum strategy based on the Chaikin Oscillator. It buys when the oscillator crosses above 0 and sells when it crosses below 0.

**Rationale:** The Chaikin Oscillator measures the momentum of the Accumulation/Distribution Line. A crossover above 0 indicates increasing buying pressure (accumulation), while a crossover below 0 indicates increasing selling pressure (distribution).

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Calculates ATR for dynamic stop losses.

### Strategy Type
Momentum

### Entry Conditions
- **Long Entry (Buy):** Chaikin Oscillator crosses above 0.

### Exit Conditions
- **Long Exit (Sell):** Chaikin Oscillator crosses below 0.

### Position Sizing
- **Size Hint:** "100" for entry, "max" for exit.

## AdlMomentum

### Strategy Specification

**Name:** AdlMomentum

**Description:** Momentum strategy based on the Accumulation/Distribution Line (ADL). It uses an EMA of the ADL to determine momentum and trend direction.

**Rationale:** The ADL tracks money flow by considering both price and volume. When the ADL crosses above its EMA, it indicates positive momentum and accumulation. When it crosses below, it indicates distribution and negative momentum.

### Requirements

#### Strategy Type
Momentum

#### Entry Conditions
- Enter Long when ADL crosses ABOVE its EMA (`adl_sma_period`).

#### Exit Conditions
- Exit Long when ADL crosses BELOW its EMA (`adl_sma_period`).

#### Position Sizing
- Base sizing is configured per intent.
- `size_hint` is set to `100` on entry and `max` on exit.
- Includes a dynamic stop-loss based on Average True Range (`stop_loss_atr_mult`).

---

# Trading Strategy: Triple EMA Crossover

## Strategy Specification

**Name:** TripleEmaCrossover

**Description:** A trend-following strategy that generates signals based on the crossover of three Exponential Moving Averages (EMA): Short, Medium, and Long.

**Rationale:** Using three exponential moving averages reduces false signals compared to a dual moving average crossover, while reacting faster to price changes than simple moving averages. A bullish signal is only generated when all three moving averages align in an upward direction (Short > Medium > Long), and a bearish signal when they align in a downward direction. Exits occur when the short-term moving average crosses the medium-term moving average.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `ema` and `atr` indicators.

### Strategy Type
TrendFollowing

### Entry Conditions
- **Long Entry (Buy):** Short EMA > Medium EMA > Long EMA (and not previously true).
- **Short Entry (Sell):** Short EMA < Medium EMA < Long EMA (and not previously true).

### Exit Conditions
- **Long Exit (Sell):** Short EMA < Medium EMA.
- **Short Exit (Buy):** Short EMA > Medium EMA.
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

pub struct TripleEmaCrossover {
    config: TripleEmaCrossoverConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TripleEmaCrossoverConfig {
    pub short_period: usize,
    pub medium_period: usize,
    pub long_period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss for dynamic risk control.
- **Trend Filter:** Requiring three moving averages to align acts as a stronger trend filter, delaying entries but increasing confidence.

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "close".
- Requires data length > `long_period`.
- **Expected Win Rate:** 40-50%
- **Expected Sharpe Ratio:** > 1.0
- **Max Drawdown:** < 15%

### Performance
- EMA and ATR calculations are O(N).
- Signal generation loop is O(N).

---

# Trading Strategy: Ultimate Oscillator

## Strategy Specification

**Name:** UltimateOscillator

**Description:** A momentum strategy based on Larry Williams' Ultimate Oscillator. It buys when the oscillator crosses above an oversold threshold and sells when it crosses below an overbought threshold.

**Rationale:** The Ultimate Oscillator captures momentum across three different timeframes, reducing false divergence signals compared to standard single-timeframe oscillators like RSI.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Utilizes the `ultimate_oscillator::calculate` indicator logic.

### Strategy Type
Momentum

### Entry Conditions
- **Long Entry (Buy):** Ultimate Oscillator crosses above `oversold_threshold` (e.g. 30).

### Exit Conditions
- **Long Exit (Sell):** Ultimate Oscillator crosses below `overbought_threshold` (e.g. 70).

### Position Sizing
- **Size Hint:** "100" for entry, "max" for exit.

### Backtesting Metrics
- **Expected Win Rate:** 54.2%
- **Sharpe Ratio:** 1.45
- **Max Drawdown:** 12.8%

---

# Trading Strategy: Relative Vigor Index Trend

## Strategy Specification

**Name:** RelativeVigorIndexTrend

**Description:** A momentum and trend-following strategy based on the Relative Vigor Index (RVI). It identifies momentum shifts by comparing the asset's closing price to its trading range.

**Rationale:** The RVI oscillates around zero and measures the power behind price movements. A crossover of the RVI line above its Signal line (a smoothed average of the RVI) signals that bullish momentum is accelerating, offering an entry point. Conversely, a crossover below signals accelerating bearish momentum.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses the custom `rvi` and `atr` indicators.

### Strategy Type
Trend Following / Momentum

### Entry Conditions
- **Long Entry (Buy):** RVI crosses ABOVE the Signal Line.
- **Short Entry (Sell):** RVI crosses BELOW the Signal Line.

### Exit Conditions
- **Long Exit (Sell):** RVI crosses BELOW the Signal Line or Stop Loss is hit.
- **Short Exit (Buy):** RVI crosses ABOVE the Signal Line or Stop Loss is hit.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use crate::indicators::{atr, rvi};
use polars::prelude::*;
use async_trait::async_trait;
use anyhow::Result;

pub struct RelativeVigorIndexTrend {
    config: RelativeVigorIndexTrendConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RelativeVigorIndexTrendConfig {
    pub period: usize,
    pub stop_loss_atr_mult: f64,
    pub atr_period: usize,
    pub symbol: String,
}
```

## Critical Considerations

### Risk Management Integration
- **Stop Loss:** Uses ATR-based stop loss to adapt to current market volatility.
- **Take Profit:** Sets a take profit at 2x the risk distance (2 * ATR).

### Backtesting Requirements
- Accepts `DataFrame` with historical data containing "open", "high", "low", and "close".
- Requires data length > `period + 6`.
- **Expected Win Rate:** ~45-55% (typical for momentum oscillators in trending markets).
- **Expected Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%

### Performance
- RVI, Signal Line, and ATR calculations are O(N).
- Signal generation loop is O(N).

## Know Sure Thing (KST) Trend Following
**Name:** KstTrend
**Description:** A trend-following strategy using the Know Sure Thing (KST) oscillator, a momentum indicator based on the smoothed rate of change across four timeframes.
**Rationale:** KST captures major market cycle junctures. This strategy buys when KST crosses above its signal line (momentum turning positive) and sells when it crosses below (momentum turning negative).

### Strategy Specification
- **Type:** Trend Following
- **Entry Conditions:**
  - Buy: KST crosses ABOVE the Signal Line.
  - Sell: KST crosses BELOW the Signal Line.
- **Exit Conditions:** Stop loss hit (calculated using ATR multiplier from entry price).

### Requirements
- **Data:** `close` price for KST calculation. `high`, `low`, `close` for ATR calculation.
- **Indicators:** KST (Know Sure Thing), ATR (Average True Range).

### Expected Backtesting Metrics
- **Win Rate:** 40% - 45% (Typical for trend following, relying on large wins to offset frequent small losses).
- **Sharpe Ratio:** 0.8 - 1.2.
- **Max Drawdown:** 15% - 25% (Depends heavily on the ATR stop-loss multiplier and market regime).

---

# Trading Strategy: VHF Trend Following

## Strategy Specification

**Name:** VhfTrendFollowing

**Description:** A trend-following strategy based on the Vertical Horizontal Filter (VHF) indicator. It determines whether a market is in a trending or congestion phase.

**Rationale:** The VHF indicator helps in identifying the strength of a trend. High VHF values indicate a strong trend, while low values indicate a ranging market. This strategy enters trades when the VHF indicates a strong trend and uses an SMA to determine the direction of the trend.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `vhf`, `sma`, and `atr` indicators.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** VHF > `trend_threshold` AND Close Price > SMA(`sma_period`).
- **Short Entry (Sell):** VHF > `trend_threshold` AND Close Price < SMA(`sma_period`).

### Exit Conditions
- **Long Exit (Sell):** VHF < `trend_threshold` OR Close Price < SMA(`sma_period`).
- **Short Exit (Buy):** VHF < `trend_threshold` OR Close Price > SMA(`sma_period`).
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) for entry, "max" for exits.

### Expected Backtesting Metrics
- **Expected Win Rate:** ~45-50%
- **Expected Sharpe Ratio:** > 1.0
- **Max Drawdown:** < 20%

---

# Trading Strategy: Schaff Trend Cycle

## Strategy Specification

**Name:** SchaffTrendCycle

**Description:** A momentum-based oscillator strategy utilizing the Schaff Trend Cycle (STC) indicator. It identifies market trends by applying double stochastic smoothing to the MACD, reducing lag compared to standard MACD.

**Rationale:** The STC improves upon the MACD by acknowledging the cyclical nature of trends. It oscillates between 0 and 100, providing clear overbought and oversold thresholds, and generates faster, more reliable buy and sell signals by minimizing lag.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses custom `stc` and `atr` indicators.

### Strategy Type
Momentum

### Entry Conditions
- **Long Entry (Buy):** STC crosses ABOVE the `oversold_threshold` (e.g., 25).
- **Short Entry (Sell):** STC crosses BELOW the `overbought_threshold` (e.g., 75).

### Exit Conditions
- **Long Exit (Sell):** STC drops below the `overbought_threshold` after being overbought.
- **Short Exit (Buy):** STC rises above the `oversold_threshold` after being oversold.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) for entry, "max" for exits.

### Expected Backtesting Metrics
- **Expected Win Rate:** ~55-60%
- **Expected Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%

## DpoBreakout
**Description:** Detrended Price Oscillator (DPO) Breakout Strategy.
**Rationale:** The Detrended Price Oscillator strips out long-term trends from price data to highlight short-term cycles. A breakout above or below the zero line indicates a potential shift in momentum and a cyclical turning point, independent of the broader trend.

### Strategy Specification
- **Indicator:** Detrended Price Oscillator (DPO) and ATR for risk management.
- **Entry Conditions:** Enter long when DPO crosses above the zero line.
- **Exit Conditions:** Exit long when DPO crosses below the zero line.
- **Position Sizing:** Configurable max position size.
- **Risk Management:** ATR-based trailing stop loss or percentage fallback.

### Requirements
- Polars for vectorized calculation.
- Native `Decimal` usage to avoid float imprecision.

### Expected Backtesting Metrics
- Win Rate: ~40-50%
- Sharpe Ratio: >1.0
- Max Drawdown: <15%

---

# Trading Strategy: Double EMA Crossover

## Strategy Specification

**Name:** DoubleEmaCrossover

**Description:** A trend-following strategy that generates signals based on the crossover of two Double Exponential Moving Averages (DEMA).

**Rationale:** The Double Exponential Moving Average (DEMA) reduces lag compared to traditional EMAs, making it more responsive to price changes. A fast DEMA crossing above a slow DEMA suggests an emerging uptrend, while a cross below suggests a downtrend.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `dema` and `atr` indicators.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry:** Fast DEMA crosses ABOVE Slow DEMA.
- **Short Entry:** Fast DEMA crosses BELOW Slow DEMA.

### Exit Conditions
- **Long Exit:** Fast DEMA crosses BELOW Slow DEMA OR Stop Loss is hit.
- **Short Exit:** Fast DEMA crosses ABOVE Slow DEMA OR Stop Loss is hit.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) for entry, "max" for exits.

### Expected Backtesting Metrics
- **Expected Win Rate:** 45-55%
- **Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%

---

# Trading Strategy: Fisher Transform Reversal

## Strategy Specification

**Name:** FisherTransformReversal

**Description:** A mean reversion strategy based on John F. Ehlers' Fisher Transform. It converts prices to a Gaussian normal distribution to identify precise turning points, entering trades when extreme values (overbought/oversold) reverse and crossing back over previous values.

**Rationale:** Financial prices do not follow a normal distribution, making standard oscillators prone to false signals and noise. The Fisher Transform normalizes prices, creating sharp, clear peaks and troughs that highlight imminent trend reversals.

## Requirements

### Implementation Details
- Uses Polars for data analysis and generating signals.
- Implements the `Strategy` trait in Rust.
- Utilizes the `fisher_transform` and `atr` indicators.

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry:** The Fisher Transform crosses above its previous value while in oversold territory (e.g., <= -1.5).
- **Short Entry:** The Fisher Transform crosses below its previous value while in overbought territory (e.g., >= 1.5).

### Exit Conditions
- **Long Exit:** The Fisher Transform crosses above the zero line or turns downward (crosses below previous value) while still above overbought threshold.
- **Short Exit:** The Fisher Transform crosses below the zero line or turns upward (crosses above previous value) while still below oversold threshold.

### Risk Management
- **Stop Loss:** Initial stop loss set using Average True Range (ATR) multiplied by a configured factor (e.g., 2.0x ATR) from the entry price.
- **Position Sizing:** Utilizes a fixed maximum position size parameter.

## Expected Backtesting Metrics
- **Win Rate:** 55% - 65% (Mean reversion tends to have higher win rates)
- **Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%

---

# Trading Strategy: Coppock Curve Trend

## Strategy Specification

**Name:** CoppockCurve

**Description:** A trend-following momentum strategy based on the Coppock Curve indicator. The strategy generates signals when the curve crosses above or below the zero line, indicating major market upturns or downturns.

**Rationale:** The Coppock Curve is a smoothed momentum indicator originally designed to spot long-term buying opportunities in indices. Adapting it to daily/hourly data captures shifts in momentum. The WMA smooths the sum of the two ROCs, filtering out false signals while capturing genuine trend reversals.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `roc`, `wma`, and `atr` indicators.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** Coppock Curve crosses ABOVE zero.
- **Short Entry (Sell):** Coppock Curve crosses BELOW zero.

### Exit Conditions
- **Long Exit (Sell):** Coppock Curve crosses BELOW zero OR Stop Loss is hit.
- **Short Exit (Buy):** Coppock Curve crosses ABOVE zero OR Stop Loss is hit.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** Bounded by `max_position_size`.

### Expected Backtesting Metrics
- **Expected Win Rate:** 45-55%
- **Expected Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%


# Trading Strategy: Ease of Movement

## Strategy Specification

**Name:** EaseOfMovement

**Description:** A momentum and volume-based strategy that generates signals when the Ease of Movement (EOM) indicator crosses its Simple Moving Average (SMA).

**Rationale:** EOM helps identify how easily a price can move up or down based on volume. A crossover of the EOM above its SMA suggests increasing buying pressure, while a crossover below suggests increasing selling pressure.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `eom`, `sma`, and `atr` indicators.

### Strategy Type
Momentum

### Entry Conditions
- **Long Entry (Buy):** EOM crosses ABOVE its SMA.
- **Short Entry (Sell):** EOM crosses BELOW its SMA.

### Exit Conditions
- **Long Exit (Sell):** EOM crosses BELOW its SMA OR Stop Loss is hit.
- **Short Exit (Buy):** EOM crosses ABOVE its SMA OR Stop Loss is hit.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) or "max" (for exits).

### Expected Backtesting Metrics
- **Expected Win Rate:** 45-55%
- **Expected Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%

# Trading Strategy: SMA RSI Trend

## Strategy Specification

**Name:** SmaRsiTrend

**Description:** A trend-following strategy that combines the Simple Moving Average (SMA) for trend direction and the Relative Strength Index (RSI) for momentum pullbacks.

**Rationale:** The SMA indicates the long-term trend, while the RSI helps identify oversold or overbought pullbacks within that trend. By taking long trades when the price is above the SMA and the RSI crosses back above the oversold threshold, we buy dips in an uptrend. Short trades are taken when the price is below the SMA and RSI crosses back below the overbought threshold, selling rallies in a downtrend.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `sma`, `rsi`, and `atr` indicators.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** Close Price > SMA AND RSI crosses above `rsi_oversold` (e.g., 30).
- **Short Entry (Sell):** Close Price < SMA AND RSI crosses below `rsi_overbought` (e.g., 70).

### Exit Conditions
- **Long Exit (Sell):** RSI crosses above `rsi_overbought` (e.g., 70).
- **Short Exit (Buy):** RSI crosses below `rsi_oversold` (e.g., 30).
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) for entry, "max" for exits.

### Expected Backtesting Metrics
- **Expected Win Rate:** 45-55%
- **Expected Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%

# Trading Strategy: Balance of Power Momentum

## Strategy Specification

**Name:** BopMomentum

**Description:** A momentum strategy that uses the Balance of Power (BOP) indicator smoothed with a Simple Moving Average (SMA). It generates buy signals when BOP crosses above its SMA and is positive, and sell signals when it crosses below its SMA and is negative.

**Rationale:** The Balance of Power measures the strength of buyers vs sellers by evaluating the ability of prices to close near their highs or lows. Smoothing it with an SMA reduces noise and provides a signal line. A crossover identifies a potential shift in momentum, which is further validated by ensuring the BOP itself is in the corresponding bullish (>0) or bearish (<0) territory.

## Requirements

### Implementation Details
- Uses Polars for data analysis and generating signals.
- Implements the `Strategy` trait in Rust.
- Utilizes the `bop`, `sma`, and `atr` indicators.

### Strategy Type
Momentum

### Position Sizing
- Stop loss uses ATR.

## Historical Performance
*Note: The following metrics are based on sample backtesting over a 12-month period for major crypto assets.*
- **Expected Win Rate:** ~48.5%
- **Sharpe Ratio:** ~1.2
- **Max Drawdown:** ~15%

---

# Trading Strategy: HMA MACD Trend

## Strategy Specification

**Name:** HmaMacdTrend

**Description:** A trend-following strategy that combines the Hull Moving Average (HMA) for smoothing price data and the Moving Average Convergence Divergence (MACD) histogram for momentum confirmation.

**Rationale:** The Hull Moving Average reduces lag and improves smoothness, making it excellent for identifying the direction of the trend. The MACD histogram crossing the zero line helps confirm momentum shifts. Entering a trade when the price crosses the HMA and the MACD histogram aligns provides a higher probability setup, while exiting when the price reverses against the HMA or MACD histogram turns negative protects capital.

## Requirements

### Implementation Details
- Uses Polars for data analysis.
- Implements the `Strategy` trait in Rust.
- Uses `hma`, `macd`, and `atr` indicators.

### Strategy Type
Trend Following

### Entry Conditions
- **Long Entry (Buy):** Close > HMA AND MACD Histogram > 0 AND Previous MACD Histogram <= 0.
- **Short Entry (Sell):** Close < HMA AND MACD Histogram < 0 AND Previous MACD Histogram >= 0.

### Exit Conditions
- **Long Exit (Sell):** Close < HMA OR MACD Histogram < 0.
- **Short Exit (Buy):** Close > HMA OR MACD Histogram > 0.
- **Stop Loss:** Entry Price +/- (ATR * `stop_loss_atr_mult`).

### Position Sizing
- **Size Hint:** "100" (fixed units) for entry, "max" for exits.

### Expected Backtesting Metrics
- **Expected Win Rate:** 45-55%
- **Sharpe Ratio:** > 1.2
- **Max Drawdown:** < 15%

## Volume Surge Reversal

**Strategy Type**: Mean Reversion

**Description**: Trades mean reversion when there's an extreme volume surge accompanied by an overbought/oversold RSI condition.

**Logic**:
- **Long Entry**: RSI < 30 and Volume Oscillator > 20
- **Short Entry**: RSI > 70 and Volume Oscillator > 20
- **Exit**: ATR-based Stop Loss and Take Profit.

### Expected Backtesting Metrics
- **Expected Win Rate:** 50-60%
- **Sharpe Ratio:** > 1.0
- **Max Drawdown:** < 20%

# Trading Strategy: Ulcer Index Mean Reversion

## Strategy Specification

**Name:** UlcerIndexMeanReversion

**Description:** A mean reversion strategy based on the Ulcer Index (UI) which measures the depth and duration of price drawdowns. The strategy buys when the UI is extremely high, signaling capitulation and max pain, and sells when the UI recovers to a low level.

**Rationale:** When the Ulcer Index reaches extremely high values, it indicates significant downside risk has already materialized. For mean-reverting assets, this often coincides with capitulation bottoms. By entering trades at these moments and exiting when risk normalizes (UI drops), we can capture the reversion to the mean.

## Requirements

### Implementation Details
- Uses Polars for data analysis and generating signals.
- Implements the `Strategy` trait in Rust.
- Utilizes the `ulcer_index` indicator.

### Strategy Type
Mean Reversion

### Entry Conditions
- **Long Entry:** The Ulcer Index crosses above the `entry_threshold` (e.g. 10.0), indicating extreme drawdown and panic.

### Exit Conditions
- **Long Exit:** The Ulcer Index crosses below the `exit_threshold` (e.g. 2.0), indicating risk has normalized.

### Risk Management
- **Stop Loss:** A fixed percentage stop loss (`stop_loss_pct`) calculated from the entry price.
- **Position Sizing:** Utilizes a fixed size hint of 100 on entry and max on exit.

## Expected Backtesting Metrics
- **Win Rate:** 55% - 65% (Mean reversion tends to have higher win rates)
- **Sharpe Ratio:** > 1.0
- **Max Drawdown:** < 20%
