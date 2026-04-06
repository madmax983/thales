# 🔭 Vantage: Spec for Verbose Empty Signals

## 👤 User Story
As a CLI User, I want clear warnings when a command successfully runs but produces no data, so that I don't mistake empty output for a silent failure.

## 💡 So What?
When users run a command like `generate-signals` and receive an empty JSON array `[]` without explanation, they often assume the tool is broken or their input is invalid. This silent failure state causes frustration and wastes time debugging correctly functioning code. By providing explicit, verbose warnings when expected data is absent—especially in interactive terminal sessions—we increase confidence in the tool and reduce onboarding friction. Complexity is a cost; clarity is revenue.

## 📈 Metric Definition
Success = 100% of `generate-signals` executions that result in zero triggered signals must output an explicit explanatory message in the `warnings` field of the JSON envelope, and when run interactively (TTY), a prominent warning must be printed to `stderr`.

## 🔍 Gap Analysis
Currently, if `generate-signals` evaluates market data but no trading condition is met on the latest candle, the CLI outputs a successful JSON envelope with an empty `data` array (`{"status":"ok", "data":[]}`). While technically correct, this is bad UX. Competing CLI tools (like linters or search tools) explicitly state when no results are found, guiding the user. We need to enhance our JSON envelope warnings and interactive `stderr` output.

## ✅ Acceptance Criteria
- Must append an explanatory string to the `warnings` array in the JSON envelope when no signals are generated (e.g., "No signals triggered for latest candle").
- Must detect if the command is being run interactively (TTY).
- If running interactively and no signals are generated, must print a prominent warning to `stderr` (e.g., "No signals generated").

## 🚫 Out of Scope
- Changing the core logic of strategy evaluation or signal generation.
- Modifying the JSON output structure beyond adding to the `warnings` array.
