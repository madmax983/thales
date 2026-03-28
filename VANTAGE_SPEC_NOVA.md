# 🔭 Vantage: Spec for Nova Feature Discovery

## 👤 User Story
As a Developer or Trader, I want clear documentation and CLI hints regarding experimental "Nova" features, so that I can easily discover and utilize them without encountering misleading "unrecognized subcommand" errors.

## 💡 So What?
If users attempt to use commands documented in our help text but receive "unrecognized subcommand" errors because they lack a feature flag, they assume the CLI is broken. This breaks trust and reduces adoption of our new, experimental features. By explicitly gating these commands with clear error messaging or documentation, we guide users to correctly enable them, increasing feature engagement and reducing support confusion.

## 📈 Metric Definition
Success = 100% of documentation (e.g., `README.md` or CLI help) referencing Nova-specific commands explicitly states the requirement to pass the `--features nova` flag. Users attempting to run these commands without the flag receive an actionable hint rather than a generic "unrecognized subcommand" error, or the commands are completely hidden from the default help menu.

## 🔍 Gap Analysis
Currently, experimental commands like `simulate-black-swan` are present in the CLI but are conditionally compiled behind the `nova` feature flag. However, users discover these commands (perhaps through source code or partial docs) and try to run them, resulting in confusing errors because the CLI behaves as if the command does not exist. Competing tools handle experimental features either by hiding them completely until a flag is passed, or by displaying them with a clear "Requires Feature X" banner.

## ✅ Acceptance Criteria
- Must update the `README.md` to include a prominent banner or note stating that experimental commands require the `--features nova` flag.
- (Optional/Engineering Decision) The CLI help text should either clearly omit these commands when the feature is disabled or provide an actionable error message if attempted.

## 🚫 Out of Scope
- Removing the `nova` feature flag entirely.
- Automatically enabling `nova` features for all users.
