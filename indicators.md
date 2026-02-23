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
