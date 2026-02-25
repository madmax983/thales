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
