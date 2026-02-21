# AGENTS.md

Guide for autonomous agents using `thales-cli`.

## What This CLI Is

`thales-cli` is a JSON-first command toolkit for trading task pipelines.

- Input: JSON files or command flags
- Output: JSON envelope on stdout
- Exit code: `0` on success, non-zero on validation/provider failure

## Build And Run

From repo root:

```powershell
cargo run -p thales-cli -- --help
```

Run a command:

```powershell
cargo run -p thales-cli -- fetch-market-data --provider alpaca --symbol AAPL --timeframe 1m
```

## Output Envelope Contract

All commands return:

```json
{
  "status": "ok | error",
  "errors": [],
  "warnings": [],
  "data": {}
}
```

- On failure, `status` is `error`, `errors` is populated, and process exits non-zero.

## Command Reference

### `fetch-market-data`

```powershell
cargo run -p thales-cli -- fetch-market-data --provider <alpaca|kraken> --symbol <SYMBOL> --timeframe <TF>
```

- Returns `BarSeries` envelope.
- Current behavior: scaffold data (single synthetic bar), not live market fetch yet.

### `normalize-bars`

```powershell
cargo run -p thales-cli -- normalize-bars --input <path-to-bars-json>
```

- Reads `BarSeries`, sorts by `timestamp_unix_ms`, returns normalized series.

### `generate-trade-intent`

```powershell
cargo run -p thales-cli -- generate-trade-intent --market <equities|crypto> --symbol <SYMBOL> --side <buy|sell> --size-hint <QTY> --confidence <0..1>
```

- Returns `TradeIntent`.
- `intent_id` format: `<market>:<symbol>:<side>:v0`.

### `validate-intent`

```powershell
cargo run -p thales-cli -- validate-intent --input <path-to-intent-json>
```

Validation rules:
- `schema_version == "v0"`
- `symbol` is non-empty
- `side` is `buy` or `sell`
- `confidence` is in `[0,1]`

### `execute-intent`

```powershell
cargo run -p thales-cli -- execute-intent --provider <alpaca|kraken> --input <path-to-intent-json>
```

- Validates intent first, then submits live order request to selected provider adapter.
- Returns `ExecutionResult` including `provider_order_id`.

## Required Environment Variables

### Alpaca
- `ALPACA_API_KEY`
- `ALPACA_API_SECRET`
- `ALPACA_BASE_URL`

### Kraken
- `KRAKEN_API_KEY`
- `KRAKEN_API_SECRET` (base64-encoded value from Kraken key settings)
- `KRAKEN_BASE_URL` (optional, defaults to `https://api.kraken.com`)

## Recommended Agent Workflow

```powershell
cargo run -p thales-cli -- fetch-market-data --provider alpaca --symbol AAPL --timeframe 1m > artifacts/fetch.json
cargo run -p thales-cli -- normalize-bars --input artifacts/fetch.json > artifacts/bars.json
cargo run -p thales-cli -- generate-trade-intent --market equities --symbol AAPL --side buy --size-hint 1 --confidence 0.7 > artifacts/intent.json
cargo run -p thales-cli -- validate-intent --input artifacts/intent.json
cargo run -p thales-cli -- execute-intent --provider alpaca --input artifacts/intent.json > artifacts/execution.json
```

## Notes For Scheduled VM Tasks

- Treat runs as ephemeral and stateless.
- Persist JSON artifacts per run for auditability.
- Never store API secrets in repo files.

## Related Docs

- `docs/runbooks/command-chaining.md`
- `docs/runbooks/scheduled-task-env.md`
- `scripts/templates/run_v0_pipeline.ps1`
