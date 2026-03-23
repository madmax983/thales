import sys
import json

issue = """
# Trading Strategy Implementation Request

## Strategy Specification

**Name:** KDJ Indicator Trading Strategy

**Description:** A mean-reversion and momentum strategy based on the KDJ indicator. It relies on the fast %K line, slow %D line, and divergence %J line to identify overbought/oversold conditions and trend reversals.

**Rationale:** KDJ extends the Stochastic Oscillator by adding the J line, which represents the divergence of %K from %D. The J line is highly sensitive to price momentum, often crossing above/below 0 or 100 before actual price reversals occur, making it a strong leading indicator.

## Requirements

### Implementation Details
- Do web research and create a new strategy.
- Use Polars for data analysis and signal generation

### Strategy Type
MeanReversion

### Entry Conditions
- Long Entry: %J line crosses above 0 (oversold reversal) OR %K crosses above %D while both are below 20.
- Short Entry: %J line crosses below 100 (overbought reversal) OR %K crosses below %D while both are above 80.

### Exit Conditions
- Long Exit: %J line crosses above 100 OR %K crosses below %D.
- Short Exit: %J line crosses below 0 OR %K crosses above %D.

### Position Sizing
- Fixed allocation of `max_position_size` per trade.

## Code Pattern

```rust
use crate::strategy::{Strategy, StrategyConfig, Signal, SignalType};
use polars::prelude::*;
use rust_decimal::Decimal;
use async_trait::async_trait;
use anyhow::Result;

pub struct KdjIndicatorStrategy {
    config: KdjIndicatorStrategyConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct KdjIndicatorStrategyConfig {
    // Strategy parameters
}

#[async_trait]
impl Strategy for KdjIndicatorStrategy {
    fn name(&self) -> &str {
        "KDJ Indicator Trading Strategy"
    }

    async fn generate_signals(&self, data: &DataFrame) -> Result<Vec<Signal>> {
        // Implementation
        todo!()
    }

    async fn update_params(&mut self, params: serde_json::Value) -> Result<()> {
        // Parameter updates
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_entry_signal_generation() {
        // Test entry signals
    }

    #[tokio::test]
    async fn test_exit_signal_generation() {
        // Test exit signals
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        // Test parameter bounds
    }
}
```

## Critical Considerations

### Risk Management Integration
- MUST include stop-loss logic
- MUST specify maximum position size
- Consider correlation with existing positions

### Backtesting Requirements
- Strategy MUST be backtestable
- Provide sample historical performance metrics
- Document expected win rate, Sharpe ratio, max drawdown

### Testing Standards (CRITICAL)
- Test signal generation logic
- Test parameter validation
- Test edge cases (no data, extreme volatility, etc.)
- Mock market data for reproducible tests
- Minimum 85% test coverage

### Performance
- Signals should generate in <100ms for typical data
- Avoid expensive calculations in hot paths
- Consider caching intermediate results

## Context
N/A

## Instructions
1. READ existing strategies
2. UNDERSTAND the Strategy requirements
3. WRITE comprehensive tests covering all signal paths
4. IMPLEMENT strategy logic
5. VERIFY backtesting compatibility
6. RUN tests
7. UPDATE strategies.md

## IMPORTANT WARNINGS
- This strategy will be used for REAL TRADING
- Code quality and testing are CRITICAL
- Any bugs could result in financial losses
- MUST be reviewed by supervisor agent before deployment
- REQUIRES human approval before going live
"""
print("Extracted details from issue text")
