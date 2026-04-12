# 🔭 Vantage: Spec for Friendly Trading Card Errors

## 👤 User Story
As a Developer or User utilizing the experimental Nova trading card export, I want readable error messages when the file output fails, so that I understand exactly which file path caused the issue without needing to decipher raw OS errors.

## 💡 So What?
If users encounter obscure errors like `os error 2` when exporting trading cards, they have no context about what went wrong (Was it the data? The template? The file path?). This high friction causes confusion and wastes time. By wrapping raw IO errors with helpful, context-aware messages that include the exact file path, we drastically improve the Developer Experience (DX) and accelerate debugging.

## 📈 Metric Definition
- **Success Criteria:** 100% of file I/O errors in `export_trading_card_svg` leak no raw OS codes in isolation. Instead, the error must clearly state the action that failed and the file path.

## 🔍 Gap Analysis
Currently, `export_trading_card_svg` bubbles up the raw `std::io::Error` code directly (e.g., `os error 2`) from `File::create(path)?`, leaving users guessing which file failed to open. Competing tools (like Ripgrep) wrap standard I/O errors with specific context (e.g., "Failed to create trading card at path '/bad/path/card.svg': No such file or directory"). We need to implement error-wrapping in our file creation logic.

## ✅ Acceptance Criteria
- Must wrap `std::io::Error` related to `File::create` in `export_trading_card_svg` using `anyhow::Context` or a custom error type to explicitly include the problematic file path (e.g., "Failed to create trading card at path '...': No such file or directory").

## 🚫 Out of Scope
- Rewriting the entire error handling framework for the CLI.
- Redesigning the SVG layout of the trading card.
