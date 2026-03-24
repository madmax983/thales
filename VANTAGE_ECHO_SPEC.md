# 🔭 Vantage: Spec for Echo: Getting Started example is confusing

## 👤 User Story
As a New User of the CLI, I want the quick start examples in the README to guarantee a non-empty output, and for the documentation to avoid confusing jargon, so that I can easily verify the tool works and understand how to use it without a finance background.

## 💡 So What?
If new users experience an empty output array on their very first quick start example, they will likely assume the tool is broken or they did something wrong, increasing frustration and decreasing adoption. Furthermore, using highly technical jargon like "OHLCV", "RAG", "TWAP", and "VWAP" without explanation creates an unnecessary barrier to entry for users who just want to use the software. By providing a foolproof getting started example and clear, simple language, we ensure a smooth onboarding experience, leading to higher retention and user satisfaction.

## 📈 Metric Definition
Success = The `README.md` `generate-signals` quick start example uses a provided `dummy_data.json` that is mathematically guaranteed to trigger a signal for the `BollingerBands` strategy, ensuring a non-empty JSON output. Additionally, 100% of the specified jargon terms ("OHLCV", "RAG", "TWAP", "VWAP") are either removed or clearly explained in simple terms within the documentation.

## 🔍 Gap Analysis
Currently, the `README.md` getting started example relies on live or recently fetched market data. Because the `generate-signals` command only outputs if a condition is met on the *exact latest candle*, this often results in an empty output `[]` if the market is moving sideways, causing confusion. Furthermore, the documentation currently assumes a high level of domain knowledge, using acronyms without simple explanations. We need to introduce a reliable mock dataset for the quick start and update the documentation's language.

## ✅ Acceptance Criteria
- Must provide a `dummy_data.json` file in the repository.
- The `dummy_data.json` must be mathematically guaranteed to trigger a signal for the `BollingerBands` strategy when processed by the `generate-signals` command.
- Must update the `README.md` `generate-signals` quick start example to use the new `dummy_data.json` (or alternatively, change the first example to use the `backtest` command).
- Must remove or explain jargon in the documentation: "OHLCV" (explain as "price data"), "RAG" (explain as "search history"), and "TWAP"/"VWAP" (explain as "time/volume spreading").

## 🚫 Out of Scope
- Modifying the underlying logic of the `generate-signals` command to output historical signals (this is what `backtest` is for).
- Rewriting all documentation from scratch.
