# 🔭 Vantage: Spec for JSON Envelope Overhead (--raw flag)

## 👤 User Story
As a CLI User, I want a `--raw` flag, so that I can pipe data directly between tools without manual JSON unwrapping.

## 💡 So What?
It improves the developer experience by removing friction when chaining commands.

## 📈 Metric Definition
Success = 100% of CLI commands that output data support a `--raw` flag.

## 🔍 Gap Analysis
Currently, all CLI output is wrapped in a JSON envelope `{"status": "ok", "data": ...}`. This requires tools like `jq` to unwrap the data before piping it to other commands, adding friction.

## ✅ Acceptance Criteria
- Must support a `--raw` flag on all data-producing commands.
- Must output only the contents of the `data` field when `--raw` is used.
- Must still return a non-zero exit code and error message on stderr if the command fails.

## 🚫 Out of Scope
Implementation of the `--raw` flag in `thales-cli`.
