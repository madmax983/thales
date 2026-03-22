# 🔭 Vantage: Spec for Friendly Error Messages

## 👤 User Story
As a New User of the CLI, I want to receive clear, actionable error messages when I input incorrect file paths or provide the wrong type of JSON data, so that I can quickly fix my mistake without needing to understand low-level OS errors or internal data structures.

## 💡 So What?
If users encounter obscure errors like `os error 2` or `missing field 'intent_id'`, they become frustrated, confused, and are likely to abandon the tool. By wrapping raw IO and JSON parsing errors with helpful, context-aware messages, we drastically improve the Developer Experience (DX), reduce the learning curve, and increase user retention.

## 📈 Metric Definition
Success = 100% of "File Not Found" errors explicitly state the missing filename, and 100% of `execute-intent` JSON parsing errors provide a hint indicating the user may have passed market data instead of TradeIntents.

## 🔍 Gap Analysis
Currently, the CLI bubbles up raw `std::io::Error` codes directly (e.g., `os error 2`), leaving users guessing which file failed. Furthermore, when `execute-intent` attempts to parse a `BarSeries` JSON file (market data) as a `TradeIntent`, it emits a low-level `serde_json` error (`missing field 'intent_id'`) instead of detecting the mismatch and guiding the user. Competing CLI tools provide explicit file references and format hints. We need to implement error-wrapping in our file reading logic.

## ✅ Acceptance Criteria
- Must wrap `std::io::Error` related to file operations to explicitly include the problematic file path (e.g., "Could not open input file 'invalid_file.json': No such file or directory").
- Must catch `serde_json` parsing failures in the `execute-intent` command and output a user-friendly hint (e.g., "Failed to parse TradeIntents. Did you pass market data instead of signals?").

## 🚫 Out of Scope
- Rewriting the entire error handling framework.
- Fixing generic validation errors outside of IO and `execute-intent` JSON parsing.
