# 🔭 Vantage: Spec for Echo: Getting Started example is confusing

## 👤 User Story
As a New User, I want the Quick Start examples to produce tangible, self-explanatory output, so that I understand how the tool works without feeling lost.

## 💡 So What?
A confusing Quick Start experience increases the abandonment rate of new developers evaluating the trading toolkit. Clear, jargon-free examples reduce time-to-value and lower the barrier to entry.

## 📈 Metric Definition
Success = 0 silent failures in the `generate-signals` quick start example, and 100% of defined jargon ("OHLCV", "RAG", "TWAP", "VWAP") in public documentation accompanied by plain-English explanations.

## 🔍 Gap Analysis
Currently, the default example fetches live data and generates signals on the latest candle. Since markets are often sideways, this frequently returns an empty array, which appears as a silent failure to users unfamiliar with the system. Additionally, the documentation uses advanced finance/crypto jargon without providing simple definitions, expecting users to already possess domain knowledge.

## ✅ Acceptance Criteria
- Must include a `dummy_data.json` that is mathematically guaranteed to trigger a signal for the `BollingerBands` strategy, OR the default quick start example must be changed to use the `backtest` command first.
- The jargon terms "OHLCV", "RAG", and "TWAP"/"VWAP" must be replaced or explained as "price data", "search history", and "time/volume spreading", respectively.

## 🚫 Out of Scope
Implementation of these fixes (Code changes, README updates, or providing the actual `dummy_data.json`).
