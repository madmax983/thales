# 🔭 Vantage: Spec for Experimental Commands

## 👤 User Story
As a New User, I want the CLI help text and documentation to explicitly state that experimental commands like `simulate-black-swan` require a feature flag, so that I don't waste time trying to run unrecognized commands.

## 💡 So What?
If users discover experimental commands in the help text but encounter an "unrecognized subcommand" error when running them, they will assume the tool is broken or the documentation is outdated. By clearly explaining the prerequisite feature flags (e.g., `--features nova`), we manage expectations and prevent unnecessary frustration, improving the overall onboarding experience.

## 📈 Metric Definition
Success = 100% of experimental commands listed in the CLI help include a clear note about requiring the `--features nova` flag, and the README has a visible banner explaining this requirement.

## 🔍 Gap Analysis
Currently, experimental commands are visible in the source code but hidden behind the `nova` feature flag at compile time. When users try to run these commands without the flag, the CLI simply states the command is unrecognized, offering no hint about how to enable it. This creates a confusing dead end for users eager to try advanced features.

## ✅ Acceptance Criteria
- Add a prominent note to the `README.md` explaining that experimental commands require the `nova` feature flag.
- Update the CLI help text for all experimental commands to explicitly mention the `nova` feature requirement.

## 🚫 Out of Scope
- Enabling the `nova` feature by default.
- Refactoring the experimental commands themselves.
