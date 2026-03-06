# 🔭 Vantage: Spec for CLI UX Improvements

## 👤 User Story
As a New CLI User, I want the help text and example documentation to be perfectly consistent, so that I can copy-paste commands without fear of errors.

## 💡 So What?
Inconsistencies between the default CLI help text and the README examples (e.g., `BollingerBands` vs `BollingerBandsMeanReversion`) create unnecessary friction, anxiety, and confusion for users trying the toolkit for the first time. Aligning them reduces time-to-value and builds trust.

## 📈 Metric Definition
Success = 0 discrepancies between the strategy used in the README's `generate-signals` example and the default strategy shown in the CLI's `--help` output.

## 🔍 Gap Analysis
Currently, the CLI help for the `generate-signals` command lists `BollingerBands` as the default strategy. However, the `README.md` example instructs users to use `BollingerBandsMeanReversion`. This discrepancy forces users to second-guess the documentation and introduces copy-paste errors.

## ✅ Acceptance Criteria
- The CLI help text for `generate-signals` must correctly reflect the strategy used in the `README.md` example (or vice versa, ensuring 100% consistency).
- If running interactively (detected via tty), the CLI must print "No signals generated" to stderr prominently when the `generate-signals` command outputs an empty array, addressing the "Silent Failure" UX issue.

## 🚫 Out of Scope
Implementation of the CLI warning logic or updating the `README.md` examples.