# 🔭 Vantage: Spec for Echo Issue Report 6

## 👤 User Story
As a New User of the CLI, I want the quick start examples in the README to guarantee a non-empty output, and for the CLI to provide clear, actionable error messages when I input incorrect file paths or provide the wrong type of JSON data, so that I can easily verify the tool works, understand how to use it, and quickly fix my mistakes without needing to understand low-level OS errors or internal data structures.

## 💡 So What?
If new users experience an empty output array on their very first quick start example, they will likely assume the tool is broken or they did something wrong, increasing frustration and decreasing adoption. Furthermore, if users encounter obscure errors like `os error 2` or `missing field 'intent_id'`, they become frustrated, confused, and are likely to abandon the tool. By providing a foolproof getting started example and wrapping raw IO and JSON parsing errors with helpful, context-aware messages, we drastically improve the Developer Experience (DX), reduce the learning curve, and increase user retention.

## 📈 Metric Definition
Success = The `README.md` `generate-signals` quick start example uses a provided `dummy_data.json` that is mathematically guaranteed to trigger a signal for the `BollingerBands` strategy, ensuring a non-empty JSON output. Additionally, 100% of "File Not Found" errors explicitly state the missing filename, and 100% of `execute-intent` JSON parsing errors provide a hint indicating the user may have passed market data instead of TradeIntents.

## 🔍 Gap Analysis
Currently, the `README.md` getting started example relies on live or recently fetched market data. Because the `generate-signals` command only outputs if a condition is met on the *exact latest candle*, this often results in an empty output `[]` if the market is moving sideways, causing confusion. Furthermore, the CLI bubbles up raw `std::io::Error` codes directly (e.g., `os error 2`), leaving users guessing which file failed. Lastly, when `execute-intent` attempts to parse a `BarSeries` JSON file (market data) as a `TradeIntent`, it emits a low-level `serde_json` error (`missing field 'intent_id'`) instead of detecting the mismatch and guiding the user. Competing CLI tools provide explicit file references and format hints. We need to introduce a reliable mock dataset for the quick start and implement error-wrapping in our file reading logic.

## ✅ Acceptance Criteria
- Must provide a `dummy_data.json` file in the repository.
- The `dummy_data.json` must be mathematically guaranteed to trigger a signal for the `BollingerBands` strategy when processed by the `generate-signals` command.
- Must wrap `std::io::Error` related to file operations to explicitly include the problematic file path (e.g., "Could not open input file 'invalid_file.json': No such file or directory").
- Must catch `serde_json` parsing failures in the `execute-intent` command and output a user-friendly hint (e.g., "Failed to parse TradeIntents. Did you pass market data instead of signals?").

## 🚫 Out of Scope
- Modifying the underlying logic of the `generate-signals` command to output historical signals (this is what `backtest` is for).
- Rewriting the entire error handling framework.
- Fixing generic validation errors outside of IO and `execute-intent` JSON parsing.
