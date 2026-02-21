# Agent Trading Toolkit V0 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Rust-first workspace that ships typed CLI tooling and provider adapter scaffolding for Alpaca + Kraken, consumable by Rust and Python agents via JSON contracts.

**Architecture:** Split the code into `contracts`, provider adapter crates, and a CLI crate. Keep provider behavior behind a shared trait boundary. Expose deterministic JSON command envelopes and validate contracts before execution. Start with mockable provider clients and contract tests.

**Tech Stack:** Rust 2024 workspace, `serde`, `serde_json`, `thiserror`, `clap`, `tokio`, `schemars`, `wiremock`.

---

### Task 1: Workspace Skeleton

**Files:**
- Modify: `Cargo.toml`
- Delete: `src/main.rs`
- Create: `crates/contracts/Cargo.toml`
- Create: `crates/contracts/src/lib.rs`
- Create: `crates/providers/alpaca/Cargo.toml`
- Create: `crates/providers/alpaca/src/lib.rs`
- Create: `crates/providers/kraken/Cargo.toml`
- Create: `crates/providers/kraken/src/lib.rs`
- Create: `crates/cli/Cargo.toml`
- Create: `crates/cli/src/main.rs`

**Step 1: Write the failing test**

Create `crates/contracts/tests/contracts_compile.rs` with a compile-level smoke test that imports contract types and asserts serialization output shape.

**Step 2: Run test to verify it fails**

Run: `cargo test -p contracts contracts_smoke_roundtrip`  
Expected: FAIL because the crate/types do not exist yet.

**Step 3: Write minimal implementation**

Create workspace and placeholder crates with minimal compileable exports.

**Step 4: Run test to verify it passes**

Run: `cargo test -p contracts contracts_smoke_roundtrip`  
Expected: PASS.

**Step 5: Commit**

```bash
git add Cargo.toml crates src
git commit -m "chore: bootstrap workspace crates for toolkit v0"
```

### Task 2: Contracts + Schema Versioning

**Files:**
- Modify: `crates/contracts/src/lib.rs`
- Create: `crates/contracts/tests/contracts_json.rs`

**Step 1: Write the failing test**

Add tests for:
- `TradeIntent` serde round-trip
- `schema_version == "v0"`
- required fields present in serialized JSON

**Step 2: Run test to verify it fails**

Run: `cargo test -p contracts contracts_json`  
Expected: FAIL on missing fields/types.

**Step 3: Write minimal implementation**

Implement core domain structs:
- `Bar`
- `BarSeries`
- `TradeIntent`
- `ExecutionRequest`
- `ExecutionResult`
- response envelope `{ status, errors, warnings, data }`

**Step 4: Run test to verify it passes**

Run: `cargo test -p contracts contracts_json`  
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/contracts
git commit -m "feat: add v0 contracts and envelope types"
```

### Task 3: Provider Interfaces + Adapter Stubs

**Files:**
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/providers/alpaca/src/lib.rs`
- Modify: `crates/providers/kraken/src/lib.rs`
- Create: `crates/providers/alpaca/tests/alpaca_adapter.rs`
- Create: `crates/providers/kraken/tests/kraken_adapter.rs`

**Step 1: Write the failing test**

Add tests that:
- construct each provider client from env-backed config
- call a mock `execute_intent` path and verify adapter returns typed `ExecutionResult`

**Step 2: Run test to verify it fails**

Run: `cargo test -p alpaca-provider` and `cargo test -p kraken-provider`  
Expected: FAIL because adapter trait/config functions are missing.

**Step 3: Write minimal implementation**

Define provider trait and minimal adapter implementations with explicit error types and placeholder transport hooks.

**Step 4: Run test to verify it passes**

Run: `cargo test -p alpaca-provider` and `cargo test -p kraken-provider`  
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/providers crates/contracts
git commit -m "feat: add provider adapter trait and v0 stubs"
```

### Task 4: CLI Commands + JSON IO

**Files:**
- Modify: `crates/cli/src/main.rs`
- Create: `crates/cli/tests/cli_contracts.rs`

**Step 1: Write the failing test**

Add tests for commands:
- `fetch-market-data`
- `normalize-bars`
- `generate-trade-intent`
- `validate-intent`
- `execute-intent`

Each test asserts JSON envelope fields and command exit behavior.

**Step 2: Run test to verify it fails**

Run: `cargo test -p thales-cli`  
Expected: FAIL because subcommands are not implemented.

**Step 3: Write minimal implementation**

Implement Clap-based subcommands and wire JSON in/out with contract types.

**Step 4: Run test to verify it passes**

Run: `cargo test -p thales-cli`  
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/cli
git commit -m "feat: add v0 cli command skeleton and json contracts"
```

### Task 5: Runbooks + Scheduler Templates

**Files:**
- Create: `docs/runbooks/scheduled-task-env.md`
- Create: `docs/runbooks/command-chaining.md`
- Create: `scripts/templates/run_v0_pipeline.ps1`

**Step 1: Write the failing test**

Add a docs check test (or CI script placeholder) verifying required env var names are documented.

**Step 2: Run test to verify it fails**

Run: `cargo test -p thales-cli docs_env_contract` (or script)  
Expected: FAIL while runbooks are absent.

**Step 3: Write minimal implementation**

Add runbooks with exact command examples and env var requirements for Alpaca + Kraken.

**Step 4: Run test to verify it passes**

Run: docs check command  
Expected: PASS.

**Step 5: Commit**

```bash
git add docs/runbooks scripts/templates
git commit -m "docs: add scheduled task runbooks and pipeline template"
```

### Task 6: Verification Gate

**Files:**
- Modify: root `Cargo.toml` (if needed for lint aliases)
- Modify: `.gitignore` (if needed)

**Step 1: Run formatting**

Run: `cargo fmt --all`

**Step 2: Run linting**

Run: `cargo clippy --all-targets --all-features -- -D warnings`

**Step 3: Run tests**

Run: `cargo test --all-targets --all-features`

**Step 4: Review for stubs**

Run: `rg "TODO|FIXME|Stub:" crates docs scripts`

**Step 5: Final commit if needed**

```bash
git add -A
git commit -m "chore: pass verification gate for toolkit v0 scaffold"
```

