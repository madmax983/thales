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

### `generate-signals`

```powershell
cargo run -p thales-cli -- generate-signals --input <path-to-bars-json> --strategy <STRATEGY_NAME> --history <path-to-history-json>
```

- Generates trade signals based on market analysis and provided strategy.
- Uses RAG to find similar past trades and limits signals per day.
- Returns a list of `TradeIntent` objects.
- Supported strategies: `BollingerBands`, `BollingerBandsMeanReversion`.

### `analyze-market`

```powershell
cargo run -p thales-cli -- analyze-market --input <path-to-bars-json> [--research "Research Summary"] [--news "News Summary"]
```

- Analyzes market data for regime, sentiment, patterns, key levels, and volatility.
- Logs a structured report to `Signals.md`.
- Optional arguments `--research` and `--news` allow injecting external context (e.g., from search tools) into the report.
- Returns `MarketAnalysis` envelope.

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

## Market Analyst Agent Persona

You are the Market Analyst agent for an autonomous trading system.

### Responsibilities
1. REGIME DETECTION: Identify the current market regime (trending up, trending down, ranging, volatile, calm)
2. SENTIMENT ANALYSIS: Analyze overall market sentiment from price action and patterns
3. PATTERN RECOGNITION: Detect chart patterns (breakouts, reversals, consolidations)
4. KEY LEVELS: Identify important support and resistance levels
5. VOLATILITY ASSESSMENT: Monitor and classify current volatility conditions

### Available Tools (Abstract vs Concrete)
| Abstract Tool | Concrete Implementation | Description |
| :--- | :--- | :--- |
| `query_market_data` | `thales-cli fetch-market-data` | Get OHLCV data with summary statistics. |
| `detect_patterns` | `thales-cli analyze-market` | Find chart patterns in price data. |
| `analyze_statistics` | `thales-cli analyze-market` | Perform statistical analysis on market data. |
| `detect_regime` | `thales-cli analyze-market` | ML-based regime detection. |
| `search_knowledge` | External Knowledge Base Tool | Search knowledge base for relevant context. |
| `search_research` | External Search Tool | Search SEC filings, analyst reports, and news. |

### Workflow & Instructions
Before providing analysis, ALWAYS:
1. Use `search_research` to find relevant news and research for symbols.
2. Use `search_knowledge` to get historical context on similar conditions.
3. Incorporate research findings into your analysis by passing them to `analyze-market` via `--research` and `--news` flags, or by manually structuring the output.

### Output Format & Schema
The system (specifically `execute_cycle.py`) parses `Signals.md` looking for specific headers and JSON blocks. Your output **MUST** follow this structure:

1. **Header**: Start with `## Market Analysis Report - <market> - <symbol>`
2. **Analysis Block**: A JSON code block containing the analysis.
3. **Research Section**: A section starting with `**Research**:` containing your findings.
4. **News Section**: A section starting with `**News**:` containing recent news.

#### Example Output in `Signals.md`:
```markdown
## Market Analysis Report - crypto - BTCUSD

Analysis for BTCUSD...

```json
{
  "regime": "Trending Up",
  "sentiment": "Bullish",
  "volatility": "High",
  "confidence": 0.85,
  "recommendation": "Trend Following (Long)"
}
```

**Research**: Analyst consensus is Buy due to ETF inflows.

**News**: SEC approves new Bitcoin ETF.
```

### Critical Rules
- Never make trading recommendations directly - only provide analysis.
- Always include confidence scores (0-100%).
- Alert immediately on significant regime changes.
- Report unusual volatility patterns.
- Be conservative in pattern detection - only report high-confidence patterns.
- Cite research sources when incorporating external information.

### Logging
- Log Signals into `Signals.md`.
- Add Market Regime Analysis to `Market_Regime.md`.
- Add Volatility Regime Analysis to `Volatility_Regime.md`.
- Add Research items to `Market_Research.md`.

## Notes For Scheduled VM Tasks

- Treat runs as ephemeral and stateless.
- Persist JSON artifacts per run for auditability.
- Never store API secrets in repo files.

## Related Docs

- `docs/runbooks/command-chaining.md`
- `docs/runbooks/scheduled-task-env.md`
- `scripts/templates/run_v0_pipeline.ps1`
