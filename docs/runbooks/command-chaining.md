# Command Chaining Runbook

This runbook shows how to chain V0 commands in scheduled jobs using JSON artifacts.

## Example Flow

1. Fetch bars:

```powershell
cargo run -p thales-cli -- fetch-market-data --provider alpaca --symbol AAPL --timeframe 1m > artifacts/fetch.json
```

2. Normalize bars:

```powershell
cargo run -p thales-cli -- normalize-bars --input artifacts/fetch.json > artifacts/bars.json
```

3. Generate intent:

```powershell
cargo run -p thales-cli -- generate-trade-intent --market equities --symbol AAPL --side buy --size-hint 1 --confidence 0.7 > artifacts/intent.json
```

4. Validate intent:

```powershell
cargo run -p thales-cli -- validate-intent --input artifacts/intent.json
```

5. Execute intent:

```powershell
cargo run -p thales-cli -- execute-intent --provider alpaca --input artifacts/intent.json > artifacts/execution.json
```

## Notes

- `status == "ok"` in output JSON indicates command success.
- Non-zero exit status indicates validation/provider failure.
- Persist `artifacts/` per run for audit and debugging.
