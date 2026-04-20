# 🔭 Vantage: Spec for CLI DX Improvements

## 1. 👤 The "User Story"
**As a** Quantitative Developer or Trader building autonomous agents,
**I want** clear, verbose feedback from CLI commands and streamlined data outputs,
**So that** I don't waste time debugging "silent failures" or writing boilerplate code to parse cumbersome JSON envelopes when analyzing trading signals.

## 2. 💡 The "So What?" (Business Value)
Currently, new users evaluating the `thales-cli` frequently encounter confusion during the "Getting Started" flow. The `generate-signals` command successfully executes but often returns an empty data array (`[]`) without prominent explanation, leading users to falsely believe the system is broken. Furthermore, the mandatory JSON envelope (`{"status": "ok", "data": [...]}`) adds unnecessary friction for users trying to pipe data directly into other CLI tools or scripts.
- **Problem:** "Silent failures" (empty data returns masquerading as success) destroy trust and increase onboarding time. Cumbersome output formats reduce the utility of the tool in Unix-style pipelines.
- **Solution:** Implement verbose, user-friendly warnings for empty signal generations and introduce a `--raw` output flag to bypass the JSON envelope.
- **Metric:** Success = 0% drop-off rate from users attempting the README "Quick Start", and a 50% reduction in support queries related to "empty signals".

## 3. ⚖️ Gap Analysis
- **Market Standard:** Standard CLI tools (like `jq`, `grep`, or mature data engineering CLIs) either provide explicit reasons for empty outputs (via stderr) or offer formatting flags (e.g., `--raw-output` or `-r`) to facilitate seamless pipeline integration.
- **Our System:** Our `generate-signals` command returns `{"status":"ok", "data":[]}` when no signal is triggered on the latest candle. While technically correct, this lacks context. To get the actual reason (e.g., "Skipping sell signal due to low confidence"), the user must actively monitor `stderr`, which is often ignored or redirected. There is also no native way to output just the data payload.

## 4. ✅ Acceptance Criteria
- **Verbose Warnings:** If the `generate-signals` command results in an empty `data` array, the JSON envelope's `warnings` array MUST contain a clear, human-readable explanation (e.g., `"warnings": ["No signals triggered for latest candle (2025-02-18 12:00)"]`).
- **Interactive Feedback:** If the CLI detects it is running interactively (attached to a TTY), it MUST prominently print a message to `stderr` indicating that no signals were generated.
- **Raw Output Flag:** All commands that output data MUST support a new `--raw` flag.
- **Raw Output Behavior:** When `--raw` is passed, the CLI MUST omit the `{"status": "ok", ...}` envelope and print only the inner JSON payload (e.g., the array of `TradeIntent` objects or the `BarSeries` array) to `stdout`.

## 5. 🚫 Out of Scope (Phase 1)
- Implementing alternative output formats like CSV, XML, or YAML.
- Modifying the underlying signal generation logic or strategy implementations.
- Refactoring the entire error handling or logging framework beyond these specific CLI surface improvements.
