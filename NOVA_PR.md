# 🌟 Nova: Analyze Market Thermodynamics

## 💡 The Spark
Financial markets behave remarkably similarly to physical systems. By modeling market data using thermodynamic principles—treating volatility as "Temperature," trading volume and order flow density as "Pressure," and momentum as "Energy"—we can gain unique macro-level insights into market states and phase transitions.

## 🚀 The Feature
Implemented the `AnalyzeThermodynamics` subcommand (behind the `nova` feature flag). This tool calculates system-level metrics based on a defined window size, exporting `ThermodynamicsReport` which includes `temperature`, `pressure`, and `energy`.

## 🔮 The Potential
This foundation allows for future modeling of market "phase transitions" (e.g., predicting crashes or breakouts by observing when market pressure exceeds temperature thresholds) or utilizing entropy metrics for regime detection.

## ⚠️ Risk
Low. Isolated in `crates/cli/src/experimental/market_thermodynamics.rs` and safely guarded behind the `#[cfg(feature = "nova")]` flag.
