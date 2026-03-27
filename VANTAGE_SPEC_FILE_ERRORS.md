# 🔭 Vantage: Spec for Friendly File Errors & Trading Jargon

## 👤 User Story
As a Developer or Trader getting started with Thales CLI, I want readable error messages for missing files and clear explanations of trading terminology, so that I can quickly understand what went wrong or what a feature does without needing a deep background in finance.

## 💼 So What?
Currently, basic file operations leak cryptic system errors like "os error 2", and financial terms like TWAP/VWAP are presented without context. This high friction causes new users to abandon the CLI immediately. Fixing this reduces onboarding friction, lowers support requests, and accelerates user activation. Complexity is a cost, and approachability is revenue.

## 📊 Metric Definition
- **Success Criteria:** 0% of file I/O errors leak raw OS codes (like "os error 2"). 100% of I/O errors contain the actionable file path.
- **Success Criteria:** 100% of documentation references to TWAP and VWAP include a plain-text explanation of their purpose.

## 🔍 Gap Analysis
- **Current State:** File reads silently drop the file path and surface standard OS errors. The documentation and CLI help text use "TWAP"/"VWAP" without defining them.
- **Market Standard:** Modern CLIs (like Cargo or Ripgrep) wrap standard I/O errors with the specific context (e.g., "Failed to read file 'foo.json': No such file or directory").

## ✅ Acceptance Criteria
- File reads must be wrapped to explicitly include the file path in the resulting error message.
- Parsing errors for configuration files or market data must be wrapped with domain-specific hints.
- The `README.md` and `AGENTS.md` must be updated to explicitly define TWAP (Time-Weighted Average Price) and VWAP (Volume-Weighted Average Price) alongside their acronyms.

## 🚫 Out of Scope
- Deprecating or renaming TWAP and VWAP internally. These are industry-standard execution algorithms; we are fixing the onboarding docs, not the engine.
- Changing how the `generate-signals` command outputs JSON on empty signals.
