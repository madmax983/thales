# 🔭 Vantage: Spec for Raw Data Output Flag

## 👤 User Story
As a Developer or Trading Bot, I want to output pure structured data without a JSON envelope, so that I can easily pipe the output directly into other CLI tools or external scripts without manual unwrapping.

## 💡 So What?
If users have to write custom parsers just to strip away our standard JSON envelope, it adds unnecessary friction to building automated pipelines. Complexity is a cost. By providing a --raw flag, we drastically improve the CLI's composability with standard Unix tools, increasing its utility and saving developers time.

## 📈 Metric Definition
Success = Users can pipe the output of commands directly into jq using a --raw flag and instantly access the underlying data without needing to reference the envelope properties.

## 🔍 Gap Analysis
Currently, all CLI commands return a wrapped JSON envelope. While useful for strict integrations, this is cumbersome for terminal workflows. Standard CLI tools often provide formatting flags to adapt to user needs. We need an option to bypass the envelope for pipeline-friendly output.

## ✅ Acceptance Criteria
- Must add a --raw flag to commands that output structured data.
- When the --raw flag is passed, the CLI must output only the raw data and omit the status/errors/warnings wrapper.
- If an error occurs while the --raw flag is used, the CLI should output the error to stderr and exit with a non-zero code.

## 🚫 Out of Scope
- Redesigning the default JSON envelope.
