---
name: Technical Indicator Implementation Request
about: Request a new technical indicator to be implemented in the strategies crate
title: 'Indicator: {{name}}'
labels: enhancement, indicator
assignees: ''

---

# Technical Indicator Implementation Request

## Indicator Specification

**Name:** {{name}}

**Description:** {{description}}

**Rationale:** {{rationale}}

## Requirements

### Implementation Details
- Create a new indicator
- Implement using Polars DataFrame interface for vectorized operations
- Use `rust_decimal::Decimal` for all calculations (NO f64)
- All timestamps must be `chrono::DateTime<Utc>`

### Input Parameters
{{inputs}}

### Output Type
{{output_type}}

### Function Signature Pattern
```rust
use polars::prelude::*;
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

pub fn calculate(
    data: &DataFrame,
    // ... parameters
) -> Result<Series> {
    // Implementation
}
```

## Code Quality Standards

### Testing (CRITICAL)
- Write comprehensive tests FIRST (TDD approach)
- Minimum 3 test cases:
  1. Known values test (with reference calculations)
  2. Edge case (empty data, single data point)
  3. Realistic market data scenario
- Test coverage target: 90%+

### Error Handling
- Use `anyhow::Result` for error returns
- Validate all inputs (check for NaN, empty data, invalid parameters)
- Provide descriptive error messages

### Documentation
- Doc comments on all public functions
- Include example usage in doc comment
- Explain the indicator's purpose and interpretation

### Performance
- Prefer vectorized Polars operations over loops
- Avoid cloning DataFrames when possible
- Use `lazy()` API for complex operations

## Example Implementation Structure

```rust
//! {{name}} - {{description}}

use anyhow::{Context, Result};
use polars::prelude::*;
use rust_decimal::Decimal;

/// Calculate {{name}}
///
/// # Arguments
/// * `data` - DataFrame with OHLCV data
/// * `period` - Lookback period
///
/// # Returns
/// Series with indicator values
///
/// # Example
/// ```rust
/// use polars::prelude::*;
/// let df = // ... load data
/// let result = calculate(&df, 14)?;
/// ```
pub fn calculate(data: &DataFrame, period: usize) -> Result<Series> {
    // Validate inputs
    if data.is_empty() {
        anyhow::bail!("Data cannot be empty");
    }

    // Implementation
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_values() {
        // Test with known reference values
    }

    #[test]
    fn test_edge_cases() {
        // Test empty data, single point, etc.
    }

    #[test]
    fn test_realistic_data() {
        // Test with realistic market data
    }
}
```

## Context
{{context}}

## Instructions
1. READ existing indicators
2. WRITE tests first in the new file
3. IMPLEMENT the indicator function
4. RUN tests
5. ENSURE all tests pass before completion
6. UPDATE indicators.md

## Critical Constraints
- NO `unwrap()` or `expect()` - use proper error handling
- NO hardcoded values without clear rationale
- NO f64 - use Decimal for all financial calculations
- MUST have tests that pass
- MUST follow existing code patterns in the crate