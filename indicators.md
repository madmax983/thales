# Technical Indicators

This document lists the technical indicators implemented in the `crates/strategies/src/indicators` module.

## Simple Moving Average (SMA)

**Name:** SMA
**Description:** Calculates the arithmetic mean of price data over a specified lookback period.
**Rationale:** Standard trend-following indicator used to smooth out price data and identify the direction of the trend.

### Implementation Details
- Uses `rust_decimal::Decimal` for all internal calculations to ensure precision.
- Returns a Polars `Series` of `f64` values for compatibility with other analysis tools.
- Handles missing data (nulls) by resetting the calculation window.

### Usage

```rust
use strategies::indicators::sma;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 14;
let sma_series = sma::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric column named "close".
- `period`: The size of the moving window (must be > 0).

### Output
- Returns `Result<Series>`.
- The output Series is named "sma".
- The first `period - 1` values will be null.

## Exponential Moving Average (EMA)

**Name:** EMA
**Description:** Calculates the Exponential Moving Average, which places a greater weight and significance on the most recent data points.
**Rationale:** Trend-following indicator that reacts more significantly to recent price changes than a simple moving average.

### Implementation Details
- Uses `rust_decimal::Decimal` for precision.
- Uses SMA of the first `period` values as the seed.
- Returns a Polars `Series` of `f64` values.

### Usage

```rust
use strategies::indicators::ema;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 14;
let ema_series = ema::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric column named "close".
- `period`: The lookback period (typically 14).

### Output
- Returns `Result<Series>`.
- The output Series is named "ema".
- The first `period - 1` values will be null.

## Relative Strength Index (RSI)

**Name:** RSI
**Description:** Calculates the Relative Strength Index, a momentum oscillator measuring the speed and change of price movements.
**Rationale:** Standard momentum indicator used to identify overbought or oversold conditions.

### Implementation Details
- Uses `rust_decimal::Decimal` for precision.
- Implements Wilder's Smoothing for the moving average calculation.
- Returns a Polars `Series` of `f64` values (0-100).
- Handles edge cases like flat prices (returns 50).

### Usage

```rust
use strategies::indicators::rsi;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 14;
let rsi_series = rsi::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric column named "close".
- `period`: The lookback period (typically 14).

### Output
- Returns `Result<Series>`.
- The output Series is named "rsi".
- The first `period` values will be null (requires `period` changes to initialize).

## Average True Range (ATR)

**Name:** ATR
**Description:** Calculates the Average True Range, a measure of market volatility.
**Rationale:** Standard volatility indicator used to determine stop loss levels and position sizing.

### Implementation Details
- Uses `rust_decimal::Decimal` for precision.
- Implements Wilder's Smoothing (RMA) for the moving average of True Range.
- Returns a Polars `Series` of `f64` values.
- True Range uses Max(High-Low, |High-PrevClose|, |Low-PrevClose|).

### Usage

```rust
use strategies::indicators::atr;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close" columns
let period = 14;
let atr_series = atr::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric columns named "high", "low", and "close".
- `period`: The lookback period (typically 14).

### Output
- Returns `Result<Series>`.
- The output Series is named "atr".
- The first `period - 1` values will be null.

## Bollinger Bands (BB)

**Name:** Bollinger Bands
**Description:** Calculates the Bollinger Bands, which consist of a middle band (SMA) and two outer bands (standard deviation away from the middle band).
**Rationale:** Technical analysis tool defined by a set of trendlines plotted two standard deviations (positively and negatively) away from a simple moving average (SMA) of a security's price.

### Implementation Details
- Uses `rust_decimal::Decimal` for all internal calculations to ensure precision.
- Implements a single-pass sliding window algorithm to calculate SMA and Standard Deviation.
- Returns a tuple of three Polars `Series` of `f64` values: (Lower, Middle, Upper).

### Usage

```rust
use strategies::indicators::bollinger_bands;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 20;
let std_dev_multiplier = 2.0;
let (lower, middle, upper) = bollinger_bands::calculate(&df, period, std_dev_multiplier)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric column named "close".
- `period`: The lookback period (typically 20).
- `std_dev_multiplier`: The number of standard deviations for the bands (typically 2.0).

### Output
- Returns `Result<(Series, Series, Series)>` representing `(lower_band, middle_band, upper_band)`.
- The output Series are named "bollinger_lower", "bollinger_middle", and "bollinger_upper".
- The first `period - 1` values will be null.

## Supertrend

**Name:** Supertrend
**Description:** Calculates the Supertrend indicator, which provides a trend direction and a trailing stop-loss line.
**Rationale:** Trend-following indicator that uses ATR to adjust for volatility and helps in identifying the primary trend direction.

### Implementation Details
- Uses `rust_decimal::Decimal` for all internal calculations to ensure precision.
- Uses ATR for volatility adjustment.
- Returns a tuple of two Polars `Series`: (Supertrend Line, Trend Direction).
- Trend Direction: 1 for Up, -1 for Down.

### Usage

```rust
use strategies::indicators::supertrend;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close" columns
let period = 10;
let multiplier = 3.0;
let (st_line, st_trend) = supertrend::calculate(&df, period, multiplier)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric columns "high", "low", and "close".
- `period`: The ATR lookback period (typically 10).
- `multiplier`: The factor to multiply ATR by (typically 3.0).

### Output
- Returns `Result<(Series, Series)>` representing `(supertrend_line, trend_direction)`.
- The output Series are named "supertrend" and "supertrend_trend".
- The first `period` values (approx) will be null.

## Donchian Channels

**Name:** Donchian Channels
**Description:** Calculates the Donchian Channels, formed by taking the highest high and the lowest low of the last `period` bars. It includes a middle band which is the average of the upper and lower bands.
**Rationale:** Trend-following indicator used to identify breakout and breakdown levels.

### Implementation Details
- Uses `rust_decimal::Decimal` for Middle Band calculation to ensure precision.
- Implements a rolling max/min window algorithm (O(N)) manually to ensure compatibility and efficiency.
- Returns a tuple of three Polars `Series` of `f64` values: (Lower, Middle, Upper).

### Usage

```rust
use strategies::indicators::donchian_channels;
use polars::prelude::*;

// Assuming df is a DataFrame with "high" and "low" columns
let period = 20;
let (lower, middle, upper) = donchian_channels::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric columns "high" and "low".
- `period`: The lookback period (typically 20).

### Output
- Returns `Result<(Series, Series, Series)>` representing `(lower_band, middle_band, upper_band)`.
- The output Series are named "donchian_lower", "donchian_middle", and "donchian_upper".
- The first `period - 1` values will be null.

## Stochastic Oscillator

**Name:** Stochastic Oscillator
**Description:** A momentum indicator comparing a particular closing price of a security to a range of its prices over a certain period of time.
**Rationale:** The Stochastic Oscillator is based on the assumption that closing prices should close near the same direction as the current trend.

### Implementation Details
- Uses `rust_decimal::Decimal` for all internal calculations to ensure precision.
- Implements Rolling Min/Max using an O(N) Monotonic Queue algorithm.
- Returns a tuple of two Polars `Series` of `f64` values: (%K, %D).

### Usage

```rust
use strategies::indicators::stochastic;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close" columns
let k_period = 14;
let k_smoothing = 3;
let d_period = 3;
let (k, d) = stochastic::calculate(&df, k_period, k_smoothing, d_period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric columns "high", "low", and "close".
- `k_period`: The lookback period for %K (typically 14).
- `k_smoothing`: The smoothing period for %K (typically 3).
- `d_period`: The smoothing period for %D (typically 3).

### Output
- Returns `Result<(Series, Series)>` representing `(percent_k, percent_d)`.
- The output Series are named "stochastic_k" and "stochastic_d".
- The first few values will be null depending on the periods.

## Keltner Channels

**Name:** Keltner Channels
**Description:** A volatility-based indicator consisting of a central Exponential Moving Average (EMA) and two outer bands derived from the Average True Range (ATR).
**Rationale:** Helps identify trend direction and potential breakouts. Price closing outside the bands suggests a strong trend.

### Implementation Details
- Uses `ema` and `atr` indicators internally.
- Uses Polars Series arithmetic for efficient calculation.
- Returns a tuple of three Polars `Series` of `f64` values: (Lower, Middle, Upper).

### Usage

```rust
use strategies::indicators::keltner_channels;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close" columns
let ema_period = 20;
let atr_period = 10;
let atr_multiplier = 2.0;
let (lower, middle, upper) = keltner_channels::calculate(&df, ema_period, atr_period, atr_multiplier)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain "high", "low", "close" columns.
- `ema_period`: Period for the central EMA (typically 20).
- `atr_period`: Period for the ATR (typically 10).
- `atr_multiplier`: Multiplier for ATR to set band width (typically 2.0).

### Output
- Returns `Result<(Series, Series, Series)>` representing `(lower_band, middle_band, upper_band)`.
- The output Series are named "keltner_lower", "keltner_middle", and "keltner_upper".

## Ichimoku Cloud

**Name:** Ichimoku Cloud (Ichimoku Kinko Hyo)
**Description:** A comprehensive indicator that defines support and resistance, identifies trend direction, gauges momentum, and provides trading signals.
**Rationale:** It provides a unique perspective on the market by showing the equilibrium of price at a glance.

### Implementation Details
- Uses `rust_decimal::Decimal` for all internal calculations (averaging) to ensure precision.
- Implements rolling Max/Min manually (O(N)) for efficiency and compatibility.
- Returns a tuple of five Polars `Series`: (Tenkan-sen, Kijun-sen, Senkou Span A, Senkou Span B, Chikou Span).
- Senkou Span A and B are shifted forward by `senkou_span_offset`.
- Chikou Span is shifted backward by `chikou_span_offset`.

### Usage

```rust
use strategies::indicators::ichimoku;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close" columns
let tenkan = 9;
let kijun = 26;
let span_b = 52;
let span_offset = 26;
let chikou_offset = 26;
let (tenkan, kijun, span_a, span_b, chikou) = ichimoku::calculate(&df, tenkan, kijun, span_b, span_offset, chikou_offset)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain "high", "low", "close" columns.
- `tenkan_period`: Lookback period for Tenkan-sen (typically 9).
- `kijun_period`: Lookback period for Kijun-sen (typically 26).
- `senkou_span_b_period`: Lookback period for Senkou Span B (typically 52).
- `senkou_span_offset`: Forward shift for Spans A and B (typically 26).
- `chikou_span_offset`: Backward shift for Chikou Span (typically 26).

### Output
- Returns `Result<(Series, Series, Series, Series, Series)>` representing `(tenkan_sen, kijun_sen, senkou_span_a, senkou_span_b, chikou_span)`.
- The output Series are named accordingly.

## On-Balance Volume (OBV)

**Name:** On-Balance Volume (OBV)
**Description:** A cumulative indicator that adds volume on up days and subtracts volume on down days.
**Rationale:** Volume precedes price. Changes in OBV can signal trend strength and potential reversals.

### Implementation Details
- Uses `rust_decimal::Decimal` for precision.
- Cumulative calculation.
- Returns a Polars `Series` of `f64` values.

### Usage

```rust
use strategies::indicators::obv;
use polars::prelude::*;

// Assuming df is a DataFrame with "close" and "volume" columns
let obv_series = obv::calculate(&df)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric columns "close" and "volume".

### Output
- Returns `Result<Series>`.
- The output Series is named "obv".

## Money Flow Index (MFI)

**Name:** Money Flow Index (MFI)
**Description:** Calculates the Money Flow Index, a momentum indicator that uses both price and volume to measure buying and selling pressure. It is often referred to as volume-weighted RSI.
**Rationale:** MFI provides a more complete picture of market sentiment than price-only indicators by incorporating volume. It helps identify overbought and oversold conditions.

### Implementation Details
- Uses `rust_decimal::Decimal` for precision.
- Calculates Typical Price (High + Low + Close) / 3.
- Calculates Raw Money Flow (Typical Price * Volume).
- Uses rolling window sums of Positive and Negative Money Flows.
- Returns a Polars `Series` of `f64` values (0-100).

### Usage

```rust
use strategies::indicators::mfi;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close", "volume" columns
let period = 14;
let mfi_series = mfi::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric columns "high", "low", "close", "volume".
- `period`: The lookback period (typically 14).

### Output
- Returns `Result<Series>`.
- The output Series is named "mfi".
- The first `period` values will be null.

## Volume Weighted Average Price (VWAP)

**Name:** VWAP
**Description:** Calculates the Volume Weighted Average Price. VWAP represents the ratio of the value traded to total volume traded over a specific time horizon.
**Rationale:** VWAP is a benchmark used by traders that provides the average price a security has traded at throughout the day, based on both volume and price. It helps to understand the true average price.

### Implementation Details
- Uses `rust_decimal::Decimal` for all calculations to maintain precision.
- Keeps a running total of the product of Typical Price ((High+Low+Close)/3) and Volume, and divides by the cumulative volume.
- Resets the cumulative values at the start of each new trading day based on the `timestamp` column.
- Returns a Polars `Series` of `String` values representing the exact `Decimal` value, as Polars has limited native support for arbitrary-precision numeric types.

### Usage

```rust
use strategies::indicators::vwap;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close", "volume" columns
let vwap_series = vwap::calculate(&df)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric columns "high", "low", "close", "volume".

### Output
- Returns `Result<Series>`.
- The output Series is named "vwap".
