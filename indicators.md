# Technical Indicators

This document lists the technical indicators implemented in the `crates/strategies/src/indicators` module.

## Simple Moving Average (SMA)

**Name:** SMA
**Description:** Calculates the arithmetic mean of price data over a specified lookback period.
**Rationale:** Standard trend-following indicator used to smooth out price data and identify the direction of the trend.

### Implementation Details
- Uses `rust_decimal::Decimal` for all internal calculations to ensure precision.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal) for compatibility with other analysis tools.
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
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

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
- Returns a Polars `Series` of `f64` values (computed internally via Decimal) (0-100).
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
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).
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

## Chandelier Exit

**Name:** Chandelier Exit
**Description:** A volatility-based indicator that uses the Average True Range (ATR) to trail a stop loss from the highest high or lowest low over a period.
**Rationale:** It helps traders ride a trend by allowing profits to run while cutting losses using a dynamic, volatility-adjusted stop level.

### Implementation Details
- Uses `rust_decimal::Decimal` for all internal calculations to ensure precision.
- Implements an O(N) rolling max/min sliding window algorithm using `std::collections::VecDeque` to maintain `<100ms` performance requirements.
- Returns a tuple of two Polars `Series` of `f64` values: (Long Exit, Short Exit).

### Usage

```rust
use strategies::indicators::chandelier_exit;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close" columns
let period = 22;
let multiplier = 3.0;
let (long_exit, short_exit) = chandelier_exit::calculate(&df, period, multiplier)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric columns "high", "low", and "close".
- `period`: The lookback period (typically 22).
- `multiplier`: Multiplier for ATR to set band width (typically 3.0).

### Output
- Returns `Result<(Series, Series)>` representing `(long_exit, short_exit)`.
- The output Series are named "chandelier_long" and "chandelier_short".
- The first `period - 1` values will be null.

## Disparity Index

**Name:** Disparity Index
**Description:** Measures the relative position of the latest closing price to a chosen moving average.
**Rationale:** Helps identify overbought or oversold conditions and potential trend reversals by quantifying the distance between the price and its moving average.

### Implementation Details
- Uses `rust_decimal::Decimal` for all internal calculations to ensure precision.
- Returns a Polars `Series` of `f64` values representing the percentage difference.
- Handles missing data and explicitly checks for nulls.

### Usage

```rust
use strategies::indicators::disparity_index;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 14;
let disparity_series = disparity_index::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric column named "close".
- `period`: The size of the moving window for the SMA (must be > 0).

### Output
- Returns `Result<Series>`.
- The output Series is named "disparity_index".
- The first `period - 1` values will be null.

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
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

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
- Returns a Polars `Series` of `f64` values (computed internally via Decimal) (0-100).

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

## Vortex Indicator

**Name:** Vortex Indicator (VI)
**Description:** A technical indicator that identifies new or existing trends in the financial markets by measuring the movement of an asset's price between two periods. It consists of two lines: VI+ (positive trend movement) and VI- (negative trend movement).
**Rationale:** The Vortex Indicator uses positive and negative trend movements to signal reversals and trend strength, based on the concept of vortex motion.

### Implementation Details
- Uses `f64` arithmetic to calculate the True Range (TR) and Vortex Movements (VM+ and VM-).
- Accumulates rolling sums of TR, VM+, and VM- over the specified period to generate the VI lines.
- Returns a tuple of two Polars `Series` of `f64` values: (VI+, VI-).

### Usage

```rust
use strategies::indicators::vortex;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close" columns
let period = 14;
let (vi_plus, vi_minus) = vortex::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric columns "high", "low", and "close".
- `period`: The lookback period (typically 14).

### Output
- Returns `Result<(Series, Series)>` representing `(vi_plus, vi_minus)`.
- The output Series are named "vi_plus" and "vi_minus".
- The first `period - 1` values will be null.

## Rate of Change (ROC)

**Name:** Rate of Change (ROC)
**Description:** A momentum oscillator that measures the percentage change in price between the current price and the price a certain number of periods ago.
**Rationale:** ROC is used to identify overbought and oversold conditions, as well as trend reversals and divergences.

### Implementation Details
- Uses `rust_decimal::Decimal` for precision.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

### Usage

```rust
use strategies::indicators::roc;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 9;
let roc_series = roc::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric "close" column.
- `period`: The lookback period (typically 9 or 14).

### Output
- Returns `Result<Series>`.
- The output Series is named "roc".
- The first `period` values will be null.

## Volume Weighted Average Price (VWAP)

**Name:** Volume Weighted Average Price (VWAP)
**Description:** A trading benchmark that gives the average price a security has traded at throughout the day, based on both volume and price.
**Rationale:** VWAP provides insight into both the trend and value of a security. It resets at the start of each trading session.

### Implementation Details
- Uses `rust_decimal::Decimal` for precision.
- Calculates Typical Price (High + Low + Close) / 3.
- Accumulates Price * Volume and Volume over the course of the day.
- Uses `timestamp_unix_ms` to detect day changes and resets the accumulators.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

### Usage

```rust
use strategies::indicators::vwap;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close", "volume", and "timestamp_unix_ms" columns
let vwap_series = vwap::calculate(&df)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric columns "high", "low", "close", "volume", and "timestamp_unix_ms".

### Output
- Returns `Result<Series>`.
- The output Series is named "vwap".

## Stochastic RSI (StochRSI)

**Name:** Stochastic RSI (StochRSI)
**Description:** Applies the Stochastic Oscillator formula to the Relative Strength Index (RSI).
**Rationale:** Standard RSI can remain between 30 and 70 for extended periods. StochRSI increases sensitivity by measuring RSI relative to its high-low range over a set period, quickly identifying extremes in the RSI itself.

### Implementation Details
- Uses `rust_decimal::Decimal` for smoothing calculations to ensure precision.
- Implements Rolling Min/Max using an O(N) Monotonic Queue algorithm over the RSI series.
- Uses sliding windows for %K and %D calculations.
- Returns a tuple of two Polars `Series` of `f64` values: (%K, %D).

### Usage

```rust
use strategies::indicators::stoch_rsi;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let rsi_period = 14;
let stoch_period = 14;
let k_period = 3;
let d_period = 3;
let (k, d) = stoch_rsi::calculate(&df, rsi_period, stoch_period, k_period, d_period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric "close" column.
- `rsi_period`: The lookback period for RSI calculation (typically 14).
- `stoch_period`: The lookback period for the Stochastic calculation on RSI (typically 14).
- `k_period`: The smoothing period for %K (typically 3).
- `d_period`: The smoothing period for %D (typically 3).

### Output
- Returns `Result<(Series, Series)>` representing `(percent_k, percent_d)`.
- The output Series are named "stoch_rsi_k" and "stoch_rsi_d".
- The first `rsi_period + stoch_period + k_period + d_period - 3` values will generally be null depending on the periods.

## TRIX

**Name:** TRIX
**Description:** Calculates the Triple Exponential Average, a momentum indicator showing the percentage rate of change of a triple exponentially smoothed moving average.
**Rationale:** Used as a momentum indicator to identify overbought and oversold markets, and as a trend indicator, effectively filtering out minor price movements.

### Implementation Details
- Uses `rust_decimal::Decimal` for all internal calculations to ensure precision.
- Uses three successive passes of the `ema` indicator on the "close" column.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal) representing the percentage rate of change.

### Usage

```rust
use strategies::indicators::trix;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 15;
let trix_series = trix::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric "close" column.
- `period`: The lookback period (typically 15 or 18).

### Output
- Returns `Result<Series>`.
- The output Series is named "trix".
- The first `period * 3` (approximate) values will be null.

## Stochastic RSI (StochRSI)

**Name:** StochRSI
**Description:** Applies the Stochastic Oscillator formula to the Relative Strength Index (RSI).
**Rationale:** Standard RSI can languish between 30 and 70 for extended periods. StochRSI is more sensitive and quickly identifies extremes in RSI itself.

### Implementation Details
- Uses a Deque algorithm to compute rolling min and max values of the RSI series efficiently in O(N).
- Smooths the raw StochRSI using Simple Moving Average (SMA) logic.
- Returns a tuple of `(Series, Series)` for `%K` and `%D`.

### Usage

```rust
use strategies::indicators::stoch_rsi;
use polars::prelude::*;

// Let `df` be a DataFrame with a "close" column
// Parameters: data, rsi_period, stoch_period, k_period, d_period
let (k_series, d_series) = stoch_rsi::calculate(&df, 14, 14, 3, 3).unwrap();
```

## Triple Exponential Moving Average (TEMA)

**Name:** TEMA
**Description:** Calculates the Triple Exponential Moving Average, an indicator designed to reduce the lag of traditional exponential moving averages.
**Rationale:** Used as an alternative to simple or exponential moving averages for trend identification and crossover signals, providing a faster response to price changes.

### Implementation Details
- Uses `f64` for all internal calculations.
- Uses three successive passes of the `ema` indicator on the "close" column.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal) representing the TEMA.

### Usage

```rust
use strategies::indicators::tema;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 9;
let tema_series = tema::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric "close" column.
- `period`: The lookback period (typically 9, 21, or 50).

### Output
- Returns `Result<Series>`.
- The output Series is named "tema".
- The first `period * 3` values will be null.

## Weighted Moving Average (WMA)

**Name:** WMA
**Description:** Calculates the Weighted Moving Average, which places a greater weight on the most recent data points.
**Rationale:** Used as a moving average that reacts more quickly to recent price changes than a Simple Moving Average (SMA), but without the exponential weighting of an EMA.

### Implementation Details
- Uses `rust_decimal::Decimal` for all internal calculations to ensure precision.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).
- Handles missing data (nulls) by resetting the calculation window or invalidating points correctly.

### Usage

```rust
use strategies::indicators::wma;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 14;
let wma_series = wma::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric "close" column.
- `period`: The lookback period (typically 9, 14, or 21).

### Output
- Returns `Result<Series>`.
- The output Series is named "wma".
- The first `period - 1` values will be null.

## Double Exponential Moving Average (DEMA)

**Name:** DEMA
**Description:** Calculates the Double Exponential Moving Average.
**Rationale:** The DEMA is designed to be a faster-moving indicator than the traditional EMA by calculating the EMA of the EMA, and then applying a specific weighting formula to reduce the lag associated with traditional moving averages.
**Implementation Details:**
- Uses the formula: `DEMA = (2 * EMA) - EMA(EMA)`.
- Implemented natively using Polars Series for speed.
**Usage:** Used in trend-following strategies like `DemaCrossover`.
**Parameters:**
- `period`: The lookback period for the exponential moving averages.

## Arnaud Legoux Moving Average (ALMA)

**Name:** ALMA
**Description:** Calculates the Arnaud Legoux Moving Average, which uses a Gaussian distribution offset to determine the weights of the moving average.
**Rationale:** It aims to reduce lag while increasing smoothness compared to traditional moving averages, avoiding overshoot by focusing the weight on the center of the window offset by a specific factor.

### Implementation Details
- Uses `rust_decimal::Decimal` for precision.
- Calculates window weights using a Gaussian function.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

### Usage

```rust
use strategies::indicators::alma;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 9;
let offset = 0.85;
let sigma = 6.0;
let alma_series = alma::calculate(&df, period, offset, sigma)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric "close" column.
- `period`: The lookback window period (must be > 0).
- `offset`: The offset factor (typically 0.85) controlling the center of the window.
- `sigma`: The standard deviation factor (typically 6.0) for the Gaussian filter.

### Output
- Returns `Result<Series>`.
- The output Series is named "alma".
- The first `period - 1` values will be null.

## Chande Momentum Oscillator (CMO)

**Name:** CMO
**Description:** Calculates the Chande Momentum Oscillator, which measures the momentum of a given asset.
**Rationale:** Used as a momentum indicator that ranges between -100 and +100 to identify overbought or oversold conditions, similar to RSI but uses differences in gains vs. losses over total movement.

### Implementation Details
- Uses `rust_decimal::Decimal` for all internal calculations to ensure precision.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

### Usage

```rust
use strategies::indicators::cmo;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 9;
let cmo_series = cmo::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric "close" column.
- `period`: The lookback period (typically 9 or 14).

### Output
- Returns `Result<Series>`.
- The output Series is named "cmo".
- The first `period` values will be null.

## Kaufman's Adaptive Moving Average (KAMA)

**Name:** KAMA
**Description:** Calculates the KAMA, an intelligent moving average that adapts to market noise or volatility. It closely follows prices when price swings are relatively small and noise is low, and adjusts to moving averages when prices swing widely and noise is high.
**Rationale:** It improves upon traditional moving averages by dynamically adjusting the smoothing factor based on market efficiency, thereby reducing false signals and lag.

### Implementation Details
- Uses `rust_decimal::Decimal` for precision internally.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

### Usage

```rust
use strategies::indicators::kama;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 10;
let fast_ema_period = 2;
let slow_ema_period = 30;
let kama_series = kama::calculate(&df, period, fast_ema_period, slow_ema_period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric "close" column.
- `period`: The Efficiency Ratio (ER) lookback period (typically 10).
- `fast_ema_period`: Fast EMA period (typically 2).
- `slow_ema_period`: Slow EMA period (typically 30).

### Output
- Returns `Result<Series>`.
- The output Series is named "kama".
- The first `period` values will be null.

## Force Index (FI)

**Name:** Force Index
**Description:** Alexander Elder's Force Index combines price movement and volume to measure the strength of bulls and bears in the market.
**Rationale:** It captures the direction, extent, and volume of price changes. A positive Force Index indicates bulls are in control, while a negative value indicates bears are in control. It's often smoothed with an Exponential Moving Average (EMA).

### Implementation Details
- Uses `rust_decimal::Decimal` for precision.
- Calculates raw Force Index as `(Close[i] - Close[i-1]) * Volume[i]`.
- Smooths the raw result using the `ema` indicator.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

### Usage

```rust
use strategies::indicators::force_index;
use polars::prelude::*;

// Assuming df is a DataFrame with "close" and "volume" columns
let period = 13;
let fi_series = force_index::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric "close" and "volume" columns.
- `period`: The EMA smoothing period (typically 1 or 13). A period of 1 returns the raw Force Index.

### Output
- Returns `Result<Series>`.
- The output Series is named "force_index".
- Depending on the period, initial values will be null.

## Volume Price Trend (VPT)

**Name:** VPT
**Description:** Calculates the Volume Price Trend, a momentum indicator that uses volume to confirm price trends or warn of potential reversals.
**Rationale:** Used as a momentum indicator that correlates volume with price changes. A rising VPT confirms an upward trend, while a divergence between VPT and price can signal a trend reversal.

### Implementation Details
- Uses `rust_decimal::Decimal` for all internal calculations to ensure precision.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).
- Cumulative calculation where current VPT = Previous VPT + Volume * ((Current Close - Previous Close) / Previous Close).

### Usage

```rust
use strategies::indicators::vpt;
use polars::prelude::*;

// Assuming df is a DataFrame with "close" and "volume" columns
let vpt_series = vpt::calculate(&df)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric "close" and "volume" columns.

### Output
- Returns `Result<Series>`.
- The output Series is named "vpt".
- The first value will be null.

## Accumulation/Distribution Line (ADL)

**Name:** Accumulation/Distribution Line (ADL)
**Description:** A volume-based indicator designed to measure the cumulative flow of money into and out of an asset.
**Rationale:** It helps assess whether the asset is being accumulated (bought) or distributed (sold). Divergences between the ADL and the asset's price often signal an impending trend reversal.

### Implementation Details
- Uses `rust_decimal::Decimal` for precision.
- Calculates Money Flow Multiplier (MFM) for each period based on high, low, and close.
- Calculates Money Flow Volume (MFV) by multiplying MFM with volume.
- Cumulatively sums the MFV to form the Accumulation/Distribution Line.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

### Usage

```rust
use strategies::indicators::adl;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close", and "volume" columns
let adl_series = adl::calculate(&df)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric "high", "low", "close", and "volume" columns.

### Output
- Returns `Result<Series>`.
- The output Series is named "adl".

## Volume Oscillator

**Name:** Volume Oscillator
**Description:** Calculates the difference between two moving averages of volume, expressed as a percentage.
**Rationale:** Identifies whether volume trend is increasing or decreasing, confirming price trends.

### Implementation Details
- Uses `rust_decimal::Decimal` (via SMA) for calculations to ensure precision.
- Returns a Polars `Series` of `f64` values.
- Positive values indicate increasing short-term volume.

### Usage

```rust
use strategies::indicators::volume_oscillator;
use polars::prelude::*;
```

## Percentage Price Oscillator (PPO)

**Name:** PPO
**Description:** Calculates the Percentage Price Oscillator, which measures the percentage difference between two moving averages (typically exponential).
**Rationale:** Similar to MACD, but normalized as a percentage. This allows for comparing the indicator across different assets or different timeframes of the same asset, as it is not affected by the absolute price level.

### Implementation Details
- Uses `rust_decimal::Decimal` iteratively for all math calculations to ensure financial precision, explicitly converting from and to `f64` only for Polars boundaries.
- Built on top of the `ema` indicator.
- Formula: `((Fast EMA - Slow EMA) / Slow EMA) * 100`.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

### Usage

```rust
use strategies::indicators::ppo;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let fast_period = 12;
let slow_period = 26;
let signal_period = 9;
let (ppo_line, signal_line, histogram) = ppo::calculate(&df, fast_period, slow_period, signal_period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric "close" column.
- `fast_period`: Fast EMA period (typically 12).
- `slow_period`: Slow EMA period (typically 26).
- `signal_period`: Signal Line EMA period (typically 9).

### Output
- Returns `Result<(Series, Series, Series)>` representing the PPO Line, Signal Line, and Histogram.
- Series are named "ppo_line", "ppo_signal", and "ppo_hist".
- Initial values will be null until the slowest moving average has enough data.

## Choppiness Index (CHOP)

**Name:** CHOP
**Description:** Calculates the Choppiness Index, a volatility indicator designed to determine if the market is choppy (trading sideways) or not choppy (trading within a trend in either direction).
**Rationale:** It helps traders determine whether to use trend-following or mean-reverting strategies based on the current market condition. A higher value indicates choppiness, while a lower value indicates a strong trend.

### Implementation Details
- Uses `rust_decimal::Decimal` iteratively for all math calculations to ensure financial precision, explicitly converting from and to `f64` only for Polars boundaries.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

### Usage

```rust
use strategies::indicators::choppiness_index;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close" columns
let period = 14;
let chop_series = choppiness_index::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric "high", "low", and "close" columns.
- `period`: The lookback period (typically 14).

### Output
- Returns `Result<Series>`.
- The output Series is named "choppiness_index".
- The first `period - 1` values will be null.

## Chaikin Oscillator

**Name:** Chaikin Oscillator
**Description:** Calculates the Chaikin Oscillator, which measures the momentum of the Accumulation/Distribution Line (ADL) using the MACD formula.
**Rationale:** It helps traders anticipate changes in the ADL by measuring its momentum. A cross above zero indicates buying pressure, while a cross below zero indicates selling pressure. It is often used to spot divergences with price.

### Implementation Details
- Uses `rust_decimal::Decimal` iteratively for all math calculations to ensure financial precision, explicitly converting from and to `f64` only for Polars boundaries.
- Built on top of the `adl` and `ema` indicators.
- Formula: `Fast EMA of ADL - Slow EMA of ADL`.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

### Usage

```rust
use strategies::indicators::chaikin_oscillator;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", "close", "volume" columns
let fast_period = 3;
let slow_period = 10;
let chaikin_series = chaikin_oscillator::calculate(&df, fast_period, slow_period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric "high", "low", "close", and "volume" columns.
- `fast_period`: Fast EMA period (typically 3).
- `slow_period`: Slow EMA period (typically 10).

### Output
- Returns `Result<Series>`.
- The output Series is named "chaikin_oscillator".
- Initial values will be null until the slow EMA has enough data points to compute.

## Ultimate Oscillator (UO)

**Name:** UO
**Description:** Calculates the Ultimate Oscillator, a momentum indicator designed by Larry Williams. It measures buying pressure across three different timeframes to reduce false divergence signals.
**Rationale:** It improves upon traditional oscillators by combining three different time periods (typically 7, 14, and 28). This helps to avoid the early divergence signals that often plague single-period oscillators. Readings below 30 denote oversold conditions, and readings above 70 denote overbought conditions.

### Implementation Details
- Uses `rust_decimal::Decimal` iteratively for all math calculations to ensure financial precision, explicitly converting from and to `f64` only for Polars boundaries.
- True Range and Buying Pressure are calculated. Null/missing values are properly propagated without corrupting calculations.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

### Usage

```rust
use strategies::indicators::ultimate_oscillator;
use polars::prelude::*;

// Assuming df is a DataFrame with "high", "low", and "close" columns
let period1 = 7;
let period2 = 14;
let period3 = 28;
let uo_series = ultimate_oscillator::calculate(&df, period1, period2, period3)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain numeric "high", "low", and "close" columns.
- `period1`: Short lookback period (typically 7).
- `period2`: Medium lookback period (typically 14).
- `period3`: Long lookback period (typically 28). `period1 < period2 < period3` must hold true.

### Output
- Returns `Result<Series>`.
- The output Series is named "ultimate_oscillator".
- Initial values up to `period3` will be null until enough data is gathered.

## ZLEMA

**Name:** ZLEMA
**Description:** Zero Lag Exponential Moving Average.
**Rationale:** A variation of EMA that reduces lag by adding momentum over a specific lag period.

### Implementation Details
- Uses `rust_decimal::Decimal`.
- Returns a Polars `Series` of `f64` values.

### Usage

```rust
use strategies::indicators::zlema;
use polars::prelude::*;
```

## Vertical Horizontal Filter (VHF)

**Name:** VHF
**Description:** Calculates the Vertical Horizontal Filter (VHF), which determines whether prices are in a trending phase or a congestion phase.
**Rationale:** VHF measures the degree to which prices are trending. A rising VHF indicates a developing trend, while a falling VHF suggests the market is entering a congestion phase. It helps traders decide whether to use trend-following indicators or oscillators.

### Implementation Details
- Uses `rust_decimal::Decimal` iteratively for all math calculations to ensure financial precision, explicitly converting from and to `f64` only for Polars boundaries.
- Formula: `(Highest Close - Lowest Close) / Sum of absolute price changes over period`.
- Returns a Polars `Series` of `f64` values (computed internally via Decimal).

### Usage

```rust
use strategies::indicators::vhf;
use polars::prelude::*;

// Assuming df is a DataFrame with a "close" column
let period = 28;
let vhf_series = vhf::calculate(&df, period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric "close" column.
- `period`: The lookback period (typically 28).

### Output
- Returns `Result<Series>`.
- The output Series is named "vhf".
- The first `period` values will be null.

## Know Sure Thing (KST)

**Name:** KST
**Description:** Calculates the Know Sure Thing oscillator, a momentum oscillator based on the smoothed rate of change for four different timeframes.
**Rationale:** KST identifies major stock market cycle junctures by capturing price momentum across four different time cycles.

### Implementation Details
- Uses `rust_decimal::Decimal` for calculations.
- Returns a tuple of two Polars `Series` of `f64` values: the KST line and its Signal line.

### Usage

```rust
use strategies::indicators::kst;
use polars::prelude::*;

// Assuming df is a DataFrame with "close" column
let roc_periods = [10, 15, 20, 30];
let sma_periods = [10, 10, 10, 15];
let signal_period = 9;
let (kst_line, kst_signal) = kst::calculate(&df, roc_periods, sma_periods, signal_period)?;
```

### Parameters
- `data`: Reference to a Polars `DataFrame`. Must contain a numeric "close" column.
- `roc_periods`: Array of 4 lookback periods for ROC.
- `sma_periods`: Array of 4 lookback periods for smoothing the ROCs.
- `signal_period`: Lookback period for the KST signal line.

### Output
- Returns `Result<(Series, Series)>` representing the KST Line and Signal Line.
- Series are named "kst" and "kst_signal".
