1. **Understand Requirements & Read Context**: Read `indicators.md` and `strategies.md` to ensure no conflict. We will implement the `StochRsiMeanReversion` strategy.
   - Requires adding a new indicator `stoch_rsi.rs` in `crates/strategies/src/indicators/`.
   - Requires adding a new strategy `stoch_rsi_mean_reversion.rs` in `crates/strategies/src/`.
   - Must follow TDD: write tests first, covering 90%+ edge cases, parameter validation, empty data.
   - MUST use `rust_decimal::Decimal` for all internal math, and `anyhow::Result` for error returns (no unwrap/expect).
   - Expected Win Rate, Sharpe Ratio, Max Drawdown must be documented in `strategies.md`.

2. **Implement `stoch_rsi` indicator**:
   - `calculate(data: &DataFrame, rsi_period: usize, stoch_period: usize, k_period: usize, d_period: usize) -> Result<(Series, Series)>`
   - Calculate RSI using `crate::indicators::rsi::calculate` or inline. Actually, `rsi::calculate` takes `&DataFrame` and `period`, returning `Series`. We can do `let rsi_series = rsi::calculate(data, rsi_period)?;`
   - Convert RSI series to `f64`, find Rolling Min/Max over `stoch_period`.
   - Calculate StochRSI = `(RSI - Min RSI) / (Max RSI - Min RSI) * 100`.
   - Calculate `%K` by smoothing StochRSI over `k_period` (typically SMA).
   - Calculate `%D` by smoothing `%K` over `d_period` (typically SMA).
   - Write tests for `stoch_rsi.rs`.
   - Register in `indicators/mod.rs`.
   - Add documentation to `indicators.md`.

3. **Implement `stoch_rsi_mean_reversion.rs` strategy**:
   - Define `StochRsiMeanReversion` and `StochRsiMeanReversionConfig` with fields: `rsi_period`, `stoch_period`, `k_period`, `d_period`, `oversold_threshold`, `overbought_threshold`, `stop_loss_atr_mult`, `atr_period`, `symbol`.
   - Implement `Strategy` trait returning `StrategyType::MeanReversion`.
   - In `generate_signals`:
     - Long Entry: `%K` crosses above `%D` and `%K` < `oversold_threshold`
     - Short Entry: `%K` crosses below `%D` and `%K` > `overbought_threshold`
     - Long Exit: Stop Loss hit (ATR based) OR `%K` crosses below `%D` above `overbought_threshold`.
     - Short Exit: Stop Loss hit (ATR based) OR `%K` crosses above `%D` below `oversold_threshold`.
   - Sizing: `100` for entry, `max` for exit.
   - Write unit tests for the strategy verifying correct entries, exits, empty data, parameter validation.
   - Register in `crates/strategies/src/lib.rs`.

4. **Integrate into `thales-cli`**:
   - Add to `crates/cli/src/strategy_factory.rs` in `create_strategy` and `list_strategies`.
   - Add to `test_execute_cycle.py` and `execute_cycle.py` active strategy list.
   - Update `strategies.md` with documentation.

5. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done**:
   - Run `pre_commit_instructions` tool and follow the steps before submission.

6. **Submit**:
   - Submit the PR.
