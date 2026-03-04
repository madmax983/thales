1. **Explore & Contextualize**: Check if `stoch_rsi.rs` exists. (Verified: it does not. The plan is clear and `rsi::calculate` returns a `Series` of f64 values).
2. **Implement `stoch_rsi` indicator**:
   - Create `crates/strategies/src/indicators/stoch_rsi.rs`.
   - Implement `calculate(data: &DataFrame, rsi_period: usize, stoch_period: usize, k_period: usize, d_period: usize) -> Result<(Series, Series)>`.
   - Call `crate::indicators::rsi::calculate` on the `data`.
   - Calculate StochRSI logic using Polars moving min/max on the RSI series, or O(N) Deque logic.
   - Smooth `%K` and `%D` using SMA logic (either O(N) sum or `sma` indicator if we create a pseudo DataFrame). I'll write simple O(N) SMA over the slices to ensure we avoid overhead and correctly handle `None`.
   - Write tests for the indicator.
   - Edit `crates/strategies/src/indicators/mod.rs` to add `pub mod stoch_rsi;`.
   - Edit `indicators.md` to document the new indicator.
   - Run `cargo check -p strategies` to verify.
3. **Implement `StochRsiMeanReversion` strategy**:
   - Create `crates/strategies/src/stoch_rsi_mean_reversion.rs`.
   - Implement `Strategy` trait returning `StrategyType::MeanReversion`.
   - Write signal generation logic and unit tests.
   - Edit `crates/strategies/src/lib.rs` to register `pub mod stoch_rsi_mean_reversion;`.
   - Run `cargo check -p strategies` to verify.
4. **Integrate into `thales-cli` and Scripts**:
   - Edit `crates/cli/src/strategy_factory.rs` to add `StochRsiMeanReversion`.
   - Edit `test_execute_cycle.py` to test inclusion (or just rely on `execute_cycle.py` reading `strategies.md`).
   - Edit `execute_cycle.py` if it hardcodes the list. (Wait, memory says "execute_cycle.py script actively parses strategies.md and currently supports...", so I don't need to edit `execute_cycle.py` if I document it properly in `strategies.md`? I will check `execute_cycle.py` just in case).
   - Update `strategies.md` with the new strategy details.
   - Run `cargo check` and `python -m unittest test_execute_cycle.py` to verify.
5. **Run test suites**:
   - Run `cargo test -p strategies`.
   - Fix warnings with `cargo clippy --all-targets --all-features -- -D warnings`.
   - Format with `cargo fmt --all`.
6. **Pre-commit**:
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
7. **Submit**:
   - Submit the PR.
