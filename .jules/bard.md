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

## 2024-05-25 - The Undocumented Predictors
**Confusion:** The `markov_chain` and `monte_carlo` modules in `crates/cli/src/` lacked documentation. Users did not understand the configurations, reports, or how to use the main functions.
**Clarification:** Added module-level documentation and executable examples (`# Examples`) for all public structures (`MarkovConfig`, `MarkovChainReport`, `MarketState`, `MonteCarloConfig`, `MonteCarloReport`) and functions (`analyze_markov_chain`, `run_simulation`), explaining the underlying concepts and detailing `# Errors` conditions.

## 2024-05-26 - The Undocumented Nova Modules
**Confusion:** The analytical modules included under the `nova` feature (`volume_profile`, `seasonality`, `pairs_trading`, and `pattern_match` in `crates/cli/src/`) were completely undocumented. Users had no idea what configurations were required or how the complex reports (e.g., Value Area, Expected Forward Return) should be interpreted.
**Clarification:** Added comprehensive module-level documentation (`//!`) to explain the core financial concepts for each analysis tool. Added item-level documentation with executable `# Examples` to demonstrate proper usage, configuration initialization, and error handling for all public functions and data structures.

## 2024-05-27 - The Silent Reports
**Confusion:** The core analytical reporting modules (`analysis.rs` and `reporting.rs`) were largely undocumented. It was unclear how raw `BarSeries` data translated to a full `MarketAnalysis` state or how the formatted Markdown reports for `Signals.md` were generated and updated.
**Clarification:** Added comprehensive module-level documentation and executable examples (`# Examples`) for `analyze`, `generate_report`, and the other reporting helper functions to bridge the gap between technical data and human-readable narrative.

## 2024-05-28 - The Silent Simulation
**Confusion:** The core CLI execution engine (, , , , ) lacked comprehensive documentation. Users could not understand how signals translated into trades, how history pending outcomes were resolved, or how synthetic data was formulated using Geometric Brownian Motion.
**Clarification:** Added extensive module-level (`//!`) and item-level (`///`) documentation outlining the simulation loops, evaluation processes, struct definitions, and executable examples for strategy instantiation and backtesting.

## 2024-05-29 - The Silent Execution
**Confusion:** The core CLI execution engine (`signals.rs`, `optimizer.rs`, `black_swan.rs`, `fear_and_greed.rs`) lacked comprehensive documentation. Users could not understand how signals translated into trades, how history pending outcomes were resolved, or how synthetic data was formulated using Geometric Brownian Motion.
**Clarification:** Added extensive module-level (`//!`) and item-level (`///`) documentation outlining the simulation loops, evaluation processes, struct definitions, and executable examples for strategy instantiation and backtesting.

## 2024-05-28 - The Silent Simulation
**Confusion:** The core CLI execution engine (backtest.rs, benchmark.rs, history.rs, strategy_factory.rs, synthetic_data.rs) lacked comprehensive documentation. Users could not understand how signals translated into trades, how history pending outcomes were resolved, or how synthetic data was formulated using Geometric Brownian Motion.
**Clarification:** Added extensive module-level (`//!`) and item-level (`///`) documentation outlining the simulation loops, evaluation processes, struct definitions, and executable examples for strategy instantiation and backtesting.
## 2024-05-30 - The Silent Bollingers
**Confusion:** The classic Bollinger Bands strategy lacked documentation for its configuration parameters and why it was built, leaving users confused as to what the numbers meant.
**Clarification:** Documented `BollingerBandsMeanReversion` and `BollingerBandsConfig` with executable examples, outlining the meaning behind std deviations and window sizes.
## 2024-05-31 - The Silent Oscillator
**Confusion:** The Awesome Oscillator strategy lacked any documentation explaining its logic, configuration, or how the crossover system triggered signals, leaving users completely blind.
**Clarification:** Documented `AwesomeOscillator` and `AwesomeOscillatorConfig` with a storytelling approach, adding executable examples, field documentation, and an explanation of the Zero-Line Crossover mechanism.
## 2024-06-01 - The Silent Stochastic
**Confusion:** The Stochastic Oscillator strategy lacked any module or struct-level documentation, leaving users confused about the specific parameters required and how signals were generated using %K and %D crossovers.
**Clarification:** Added module-level documentation and executable examples (`# Examples`) for `StochasticOscillatorConfig` and `StochasticOscillator`, detailing the exact crossing logic for oversold and overbought zones.

## 2024-06-02 - The Silent Indicators
**Confusion:** The MACD and Williams %R strategies lacked comprehensive documentation, making their configuration parameters confusing and their underlying financial concepts opaque to new users.
**Clarification:** Added module-level documentation (`//!`) explaining the concepts behind MACD crossovers and Williams %R overbought/oversold levels, and included executable `# Examples` for both strategy configurations and their instantiation.
## 2024-06-03 - The Silent Reversion
**Confusion:** The RSI Mean Reversion strategy lacked module and struct-level documentation, leaving users confused about the specific parameters required and how signals were generated using oversold and overbought zones.
**Clarification:** Added module-level documentation and executable examples (`# Examples`) for `RsiMeanReversionConfig` and `RsiMeanReversion`, detailing the exact crossing logic for oversold and overbought zones, and configuring the ATR trailing stop loss.

## 2024-06-04 - The Silent KAMA
**Confusion:** The Kaufman's Adaptive Moving Average (KAMA) Crossover strategy lacked any module-level or struct-level documentation, making it difficult to understand its parameters or how it adjusts its smoothing constant based on market volatility to prevent whipsaws.
**Clarification:** Added module-level (`//!`) and struct-level documentation for `KamaCrossoverConfig` and `KamaCrossover`, explaining the crossover logic and including an executable `# Examples` JSON deserialization snippet.
## 2024-06-05 - The Silent Ultimate
**Confusion:** The Ultimate Oscillator strategy lacked module-level documentation, leaving users confused about the specific parameters required (periods 1, 2, 3), how they combine into a weighted calculation, and how signals were generated using oversold and overbought zones.
**Clarification:** Added module-level documentation and executable examples (`# Examples`) for `UltimateOscillatorConfig` and `UltimateOscillator`, detailing the exact crossing logic for oversold and overbought zones, and configuring the ATR trailing stop loss.

## 2024-06-06 - The Silent Crossover and The Hidden Volume
**Confusion:** The EMA Crossover and OBV Trend Following strategies lacked module-level documentation and struct documentation. Users were left to guess what configuration parameters like `short_window` and `obv_sma_period` actually meant in the context of the respective strategy concepts.
**Clarification:** Added storytelling module-level documentation to `ema_crossover.rs` and `obv_trend.rs` explaining the financial theories behind moving average crossovers and on-balance volume. Added executable doctests for configuration structs to guide proper initialization.
## 2024-06-07 - The Silent Volume
**Confusion:** The OBV Trend Following strategy lacked executable examples for its configuration and strategy initialization, making it difficult for users to understand how to set it up.
**Clarification:** Added executable doctests for `ObvTrendFollowingConfig` and `ObvTrendFollowing` structs.

## 2024-06-08 - The Silent Momentum
**Confusion:** The Chaikin Money Flow and Chaikin Oscillator Momentum strategies lacked any documentation, leaving users guessing about the exact rules that triggered signals and the rationale behind their configurations.
**Clarification:** Added module-level documentation and executable examples (`# Examples`) for `ChaikinMoneyFlowConfig`, `ChaikinMoneyFlow`, `ChaikinOscillatorMomentumConfig`, and `ChaikinOscillatorMomentum`. Detailed the exact logic for zero-line crossovers and risk management.

## 2026-03-22 - The Silent Contracts
**Confusion:** Core contracts in `crates/contracts/src/lib.rs` (like `ExecutionRequest`, `ExecutionResult`, `MarketAnalysis`, `Position`, and `Order`) lacked executable examples, making it unclear how to instantiate or test them.
**Clarification:** Added `# Examples` sections with executable doctests to all major data structures in `crates/contracts/src/lib.rs` to demonstrate typical initialization and assertions.

## 2026-03-22 - The Unmapped Fractal & SMC Regions
**Confusion:** Advanced analytical modules like `fractal_dimension.rs` and `order_blocks.rs` in `crates/cli/src/` lacked both module-level and item-level documentation with executable examples. Users could not discover how to instantiate these tools or interpret concepts like Higuchi Fractal Dimension or SMC (Smart Money Concepts) unmitigated order blocks.
**Clarification:** Added storytelling module-level documentation (`//!`) to explain the core financial and mathematical concepts. Added executable `# Examples` to all configuration structures and main functions, guiding developers on proper initialization and result interpretation.

## 2026-03-22 - The Unmapped Experimental Regions
**Confusion:** Experimental modules like `cycle_analysis.rs`, `similarity_search.rs`, and `strategy_correlation.rs` in `crates/cli/src/experimental/` lacked executable examples. Users could not discover how to instantiate configurations, analyze dominant cycles, find historical similarities, or interpret the correlation matrix between different strategies.
**Clarification:** Added storytelling module-level documentation (`//!`) to explain the core concepts. Added executable `# Examples` to all configuration structures, report definitions, and main functions (`analyze_cycles`, `find_similar_patterns`, `analyze_correlations`), guiding developers on proper initialization and result interpretation.

## 2026-03-24 - The Silent Fisher
**Confusion:** The Fisher Transform Reversal strategy lacked module-level documentation, leaving users confused about the specific parameters required, the significance of the overbought/oversold thresholds, and how it differs from a standard oscillator.
**Clarification:** Added module-level documentation and executable examples (`# Examples`) for `FisherTransformReversalConfig` and `FisherTransformReversal`. Detailed the math behind the Gaussian transformation and exactly how crossing the `overbought_threshold` or `oversold_threshold` triggers entry or exit signals.
## 2026-03-24 - The Silent Reversion
**Confusion:** The VWAP Reversion strategy lacked module-level documentation and executable examples. Users could not discover how to instantiate the configuration or interpret the overbought/oversold bands relative to the Volume Weighted Moving Average.
**Clarification:** Added storytelling module-level documentation (`//!`) to explain the core reversion concepts. Added executable `# Examples` to `VwapReversionConfig` and `VwapReversion`, guiding developers on proper initialization.
## 2026-03-29 - The Silent KDJ
**Confusion:** The KDJ Indicator Strategy lacked comprehensive documentation, making its configuration parameters confusing and its underlying concepts opaque.
**Clarification:** Added storytelling module-level documentation (`//!`) explaining the concepts behind KDJ indicator overbought/oversold levels, and included executable `# Examples` for both strategy configurations and their instantiation.

## 2024-06-09 - The Silent Spreading
**Confusion:** The jargon TWAP and VWAP were confusing users. Also, the `generate-signals` command example was returning empty outputs.
**Clarification:** Replaced jargon with plain-English explanations and updated the example to use `dummy_data.json` that guarantees a signal.

## 2024-05-31 - The Unmapped Gamification & Physics Regions
**Confusion:** The experimental modules `market_energy.rs` and `trading_card.rs` in `crates/cli/src/experimental/` were undocumented. Users had no idea how to interpret market data as potential/kinetic energy or how to generate RPG-style stats and SVGs.
**Clarification:** Added storytelling module-level documentation (`//!`) to explain the core concepts. Added executable `# Examples` to configurations and main functions (`analyze_energy`, `export_trading_card_svg`) to guide users on usage.

## 2026-04-04 - The Jargon Barrier
**Confusion:** Jargon such as TWAP, VWAP, OHLCV, and RAG were confusing to new users, but blindly replacing them with plain-English phrases broke standard industry terminology.
**Clarification:** Reverted blind text replacements. Retained the original acronyms and appended their plain-English definitions (e.g., 'VWAP (Volume Weighted Average Price)') in `README.md` and across module-level rustdocs.
