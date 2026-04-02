# 🔭 Vantage: Spec for Getting Started

* 👤 **User Story:** As a new user, I want the Quick Start examples to produce a visible output and the documentation to define financial jargon clearly, so that I can easily verify the tool works and understand the terminology.
* ✅ **Acceptance Criteria:**
  - The Quick Start example must be updated to use the backtest command or a dummy_data.json must be provided that guarantees a visible signal output.
  - Financial acronyms (e.g., OHLCV, RAG, TWAP, VWAP) must be retained but appended with their plain-English definition (e.g., "VWAP (Volume Weighted Average Price)").
* 🚫 **Out of Scope:** Changing the core logic of the generate-signals command or any underlying strategy implementations.
