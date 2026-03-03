# Bard's Journal 🎻

## 2024-05-22 - The Missing Map
**Confusion:** The project had no `README.md` at the root, making it impossible for a new developer (or agent) to understand how to build or run the CLI without digging into `AGENTS.md`.
**Clarification:** Created a developer-focused `README.md` that bridges the gap between the code and the agent documentation.

## 2024-05-22 - The Silent Contract
**Confusion:** The core data structures in `crates/contracts` (like `TradeIntent`) were completely undocumented. Users had to guess what `confidence` meant or what values `size_hint` accepted.
**Clarification:** Added detailed docstrings to `crates/contracts/src/lib.rs` explaining every field, especially "magic strings" like `size_hint="max"`.

## 2024-05-22 - The Hidden Strategy
**Confusion:** The `Strategy` trait in `crates/strategies` had no documentation. It wasn't clear what the expected input `DataFrame` should look like (required columns).
**Clarification:** documented the `Strategy` trait and explicitly listed the required columns (`open`, `high`, `low`, `close`, `volume`, `timestamp`) in the `generate_signals` method docs.

## 2025-02-28 - The Missing Indicators
**Confusion:** The technical indicators module (`crates/strategies/src/indicators/`) lacked high-level documentation describing what indicators were available. Furthermore, several core indicators like RSI, MACD, and SMA were missing executable examples and details about how they handle edge cases like missing data or insufficient lookback periods.
**Clarification:** Added a module-level docstring (`//!`) to `indicators/mod.rs` summarizing the available indicators. Added comprehensive documentation to `rsi.rs`, `macd.rs`, and `sma.rs`, including `## Examples`, `## Panics`, and `## Edge Cases` to ensure users know how the functions behave in non-ideal conditions.
