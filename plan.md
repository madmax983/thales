1. **Explore & Research**: I need to implement a new trading strategy. Since the system has an existing ROC (Rate of Change) indicator (`crates/strategies/src/indicators/roc.rs`) that is not used by any current strategy, I will implement a `RocMomentum` strategy! This aligns perfectly with the requirement to "create a new strategy" and "Use Polars for data analysis".

**Name:** RocMomentum
**Description:** A momentum strategy based on the Rate of Change (ROC) indicator. It identifies overbought and oversold conditions, as well as trend reversals, by measuring the percentage change in price over a given period.
**Rationale:** Momentum often precedes price. A high positive ROC indicates strong bullish momentum (overbought), while a low negative ROC indicates strong bearish momentum (oversold). Crossing above/below the zero line or specific thresholds can signal trend changes.
**Strategy Type:** Momentum
**Entry Conditions:**
- Long Entry (Buy): ROC crosses above `buy_threshold` (e.g., 0.0 or a slightly positive value) AND ROC > Previous ROC.
- Short Entry (Sell): ROC crosses below `sell_threshold` (e.g., 0.0 or a slightly negative value) AND ROC < Previous ROC.
**Exit Conditions:**
- Long Exit (Sell): ROC crosses below `sell_threshold` OR ROC < Previous ROC (momentum slowing down).
- Short Exit (Buy): ROC crosses above `buy_threshold` OR ROC > Previous ROC.
- Stop Loss: Entry Price +/- (ATR * `stop_loss_atr_mult`).
**Position Sizing:** "100" or "max".

Wait, let's keep it simple to ensure robustness.
Entry: ROC crosses above `buy_threshold`
Exit: ROC crosses below `sell_threshold`, or Stop Loss hit.

2. **Implement Strategy Logic (`crates/strategies/src/roc_momentum.rs`)**:
   - Define `RocMomentum` and `RocMomentumConfig`.
   - Implement `Strategy` trait.
   - Use `roc` and `atr` indicators.
   - Calculate signals using Polars `DataFrame`.
   - Write comprehensive tests inside the file (covering entry, exit, parameter validation, edge cases).

3. **Register Strategy**:
   - Add `pub mod roc_momentum;` to `crates/strategies/src/lib.rs`.
   - Add `RocMomentum` to `crates/cli/src/strategy_factory.rs`.
   - Add `RocMomentum` to `execute_cycle.py` strategy list.

4. **Update Documentation**:
   - Add `RocMomentum` section to `strategies.md` with expected metrics (Win Rate, Sharpe, Max Drawdown).

5. **Run Tests**:
   - `cargo test -p strategies`
   - `python -m unittest test_execute_cycle.py` (if it exists)
   - `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings`

6. **Pre-commit steps**: Ensure proper testing, verification, review, and reflection are done by calling `pre_commit_instructions`.

7. **Submit**: Create PR.
