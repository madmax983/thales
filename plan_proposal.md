1. **Implement Strategy Struct and Config using `write_file`**
   * Use `write_file` to create `crates/strategies/src/sma_crossover.rs`.
   * The file will define `SmaCrossover` and `SmaCrossoverConfig` (with fields: `short_period`, `long_period`, `stop_loss_atr_mult`, `atr_period`, `symbol`).
   * Include imports for `sma` and `atr` from the `indicators` module.
   * Implement the `Strategy` trait for `SmaCrossover`, covering the `generate_signals` and `update_params` methods.
   * Add a `#[cfg(test)]` module containing:
     - `test_sma_crossover_signals`: mock a Polars `DataFrame` with `close`, `high`, `low`, `timestamp_unix_ms`, and `volume` spanning enough bars for ATR and SMA. Verify that a `SignalType::Entry` with `side: "buy"` is generated on the crossover.
     - `test_empty_data`: verify `generate_signals` returns `Ok(vec![])` or an appropriate empty handling without panicking.
     - `test_update_params`: instantiate `SmaCrossoverConfig`, call `update_params` with valid serialized JSON parameters, and assert the config fields are updated.

2. **Verify file creation using `run_in_bash_session`**
   * Run `cat crates/strategies/src/sma_crossover.rs` to ensure the strategy file is written correctly.

3. **Export the strategy module using `replace_with_git_merge_diff`**
   * Use `replace_with_git_merge_diff` on `crates/strategies/src/lib.rs` to add `pub mod sma_crossover;` and its strategy documentation block: `//! - [\`sma_crossover::SmaCrossover\`] - Trend following using SMA crossovers.`.

4. **Register the strategy in the CLI using `replace_with_git_merge_diff`**
   * Use `replace_with_git_merge_diff` on `crates/cli/src/strategy_factory.rs` to include the `SmaCrossover` configuration parsing in `create_strategy` and append `"SmaCrossover"` to `list_strategies()`.

5. **Verify codebase modifications using `run_in_bash_session`**
   * Run `git diff` to ensure changes are correctly applied to `lib.rs` and `strategy_factory.rs`.

6. **Run checks using `run_in_bash_session`**
   * Run `cargo fmt --all`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings` to verify code format, correctness, and style.

7. **Complete pre-commit steps**
   * Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.

8. **Submit**
   * Use the `attempt_completion` tool to complete the task.
