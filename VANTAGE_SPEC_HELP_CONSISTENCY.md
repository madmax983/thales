# 🔭 Vantage: Spec for CLI Help Text Consistency

## 👤 User Story
As a Developer, I want the CLI help text default values to match the examples provided in the README, so that I can copy-paste commands without encountering unexpected errors or mismatched behavior.

## 💡 So What?
When default values in the CLI help do not match the primary examples in the documentation, it creates cognitive dissonance and "copy-paste anxiety" for new users. This inconsistency increases friction during onboarding, causing users to second-guess the tool and slowing down their time to value. Consistency is a hallmark of polished products; friction is a cost.

## 📈 Metric Definition
Success = 100% of default strategy arguments in the CLI help text perfectly match the corresponding examples in `README.md`.

## 🔍 Gap Analysis
Currently, the CLI help text for `generate-signals` specifies `BollingerBands` as the default strategy, while the `README.md` example instructs users to use `BollingerBandsMeanReversion`. This mismatch leads to confusion for users who refer to both sources.

## ✅ Acceptance Criteria
- Must update the CLI help text default for the `strategy` argument in the `generate-signals` command to match the `README.md` example (`BollingerBandsMeanReversion`), OR update the `README.md` to use the CLI's default (`BollingerBands`).
- Must perform a full audit of all other CLI help defaults and README examples to ensure complete alignment.

## 🚫 Out of Scope
- Rewriting the underlying strategies.
- Changing the overall markdown structure of the README.
