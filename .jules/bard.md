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

## 2024-05-23 - The Mystery of RAG
**Confusion:** The RAG module (`crates/cli/src/rag.rs`) lacked any documentation. Users had no idea how it matched past trades or how the daily signal limits worked.
**Clarification:** Added comprehensive module-level documentation and executable examples (`# Examples`) for all public structures (`HistoryEntry`, `HistoricalPerformance`) and functions (`analyze_performance`, `find_similar_trades`, `count_todays_signals`, `summarize_history`) using `tempfile` to demonstrate JSON history interactions.

## 2024-05-24 - The Hidden Entropy
**Confusion:** The `entropy` module in `crates/cli/src/entropy.rs` lacked any documentation, making it difficult for users to understand what "Shannon Entropy" means in the context of market returns or how to interpret the `normalized_entropy` output (e.g., that 1.0 means pure randomness).
**Clarification:** Added comprehensive module-level documentation and executable examples (`# Examples`) for all public structures (`EntropyConfig`, `EntropyReport`) and functions (`analyze_entropy`), explaining the information theory concepts behind the calculations and explicitly detailing `# Errors` conditions.
