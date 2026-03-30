# 🔭 Vantage: Spec for CLI DX Improvements (Echo Audit)

## 👤 User Story
As a New CLI User, I want the getting started examples to run successfully out-of-the-box and the error messages to be self-explanatory, so that I don't feel lost or assume the tool is broken.

## 💡 So What?
New users dropping off due to poor DX (empty signal arrays, cryptic JSON/IO errors, confusing financial jargon, inconsistencies in documentation) directly hurts adoption. Making the first-run experience foolproof and the error messages actionable builds immediate trust.

## 📈 Metric Definition
Success = 0 silent failures in the `generate-signals` quick start example. 100% of defined jargon (OHLCV, RAG, TWAP, VWAP) explained in plain English. `execute-intent` JSON parsing failures explicitly check for market data mistakes. File IO errors explicitly state the problematic filename. The CLI help default strategy perfectly aligns with the README.

## 🔍 Gap Analysis
Currently, the quick start fetches live data which often yields no signals on the latest candle. Errors are raw OS codes (`os error 2`) or serde failures (`missing field 'intent_id'`). The documentation uses unexplained jargon and diverges from the CLI `--help` defaults.

## ✅ Acceptance Criteria
- Must include a `dummy_data.json` mathematically guaranteed to trigger a signal for the example strategy.
- Must replace jargon: "OHLCV" -> "price data", "RAG" -> "search history", "TWAP/VWAP" -> "time/volume spreading".
- Must wrap `std::io::Error` to include the file path.
- Must wrap `serde_json` errors in `execute-intent` to hint: "Failed to parse TradeIntents. Did you pass market data instead of signals?".
- CLI `--help` default strategy and README example must match.
- Must print "No signals generated" to stderr prominently if output is empty.

## 🚫 Out of Scope
- Complete rewrite of the error handling framework.
- Modifying `generate-signals` logic to output historical signals (this is backtest's job).
