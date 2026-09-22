# Morning Scan Runbook — Thales Coordinator (paper mode, 8:00 AM CT)

One coordinator owns the run. There is no peer swarm. The coordinator sequences
deterministic CLI tools, reads web research itself, calls `judge-signals` **directly**
(never wrapped in a subagent), makes the go/no-go call, and writes the audit trail.

Paper trading only. No live trade without Mark's explicit per-action authorization.
Doing nothing is an expected outcome — most runs should end without a trade.

## 0. Environment

```bash
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=stable          # nightly cannot compile Polars 0.42
export TMPDIR="$HOME/workspace/.tmp-thales-ci"   # /tmp is a 512 MiB tmpfs; linking Polars crashes there
export PAPER_PORTFOLIO_PATH="$HOME/workspace/thales/runs/<run-id>/paper_portfolio.json"
cd ~/workspace/thales
BIN=./target/debug/thales-cli          # build once with: cargo +stable build -p thales-cli
```

Key env contract (no-credential scan):

| Step | Env needed | Without it |
|---|---|---|
| `fetch-market-data --provider paper` | none | works (SYNTHETIC bars — see §Honest limits) |
| `fetch-market-data --provider kraken` | `KRAKEN_API_KEY`, `KRAKEN_API_SECRET` | fails — even though OHLC is a public endpoint, the CLI demands credentials first |
| `fetch-market-data --provider alpaca` | `ALPACA_API_KEY`, `ALPACA_API_SECRET`, `ALPACA_BASE_URL` | fails (`ALPACA_BASE_URL` has no default) |
| `scan-market --provider kraken` | `KRAKEN_API_KEY`, `KRAKEN_API_SECRET` | fails (Ticker is public; the CLI still gates on credentials) |
| `scan-market --provider alpaca` | none | returns a STATIC watchlist, not a live scan |
| `scan-market --provider paper` | none | returns 3 static symbols |
| `judge-signals` | `TYPESAFE_API_KEY` | **fails closed** — the run stops; signals never pass ungated |
| `analyze-market --jev` | `TYPESAFE_API_KEY` | heuristic labels only without `--jev` |

## 1. State — what do we hold, what is worth looking at?

```bash
mkdir -p runs/<run-id>
$BIN get-positions --provider paper > runs/<run-id>/positions.json
$BIN scan-market --provider paper --top-n 10 > runs/<run-id>/universe.json   # static list; see §Honest limits
```

Check `positions.json`: envelope is `{"status","errors","warnings","data": [...]}`.
If status is `error`, stop the run and record the error in the ledger.

## 2. Data — fetch and normalize, one candidate at a time

For each candidate symbol from the universe (at most 3 — never chase):

```bash
$BIN fetch-market-data --provider paper --symbol BTCUSD --timeframe 1h > runs/<run-id>/fetch-BTCUSD.json
$BIN normalize-bars --input runs/<run-id>/fetch-BTCUSD.json > runs/<run-id>/bars-BTCUSD.json
```

Verify `bars-<SYM>.json` has a non-empty `data.bars` array sorted by
`timestamp_unix_ms`. If the series is empty or the latest bar is stale
(older than 2x the timeframe), **stop for that symbol and record why**.
Never fabricate bars.

## 3. Research — fresh web context (coordinator's own step)

Before generating signals, gather fresh market context for each candidate:

- Current price action, news, and catalysts from web search.
- Cite sources; record them in the audit trail.
- Research is context for `analyze-market` (`--research`, `--news`) and for the
  coordinator's own judgement. It never overrides the deterministic pipeline.

```bash
$BIN analyze-market --input runs/<run-id>/bars-BTCUSD.json \
  --research "one-paragraph research summary" \
  --news "one-paragraph news summary" \
  --no-report > runs/<run-id>/analysis-BTCUSD.json
```

Use `--no-report`: without it the CLI appends to hardcoded `./Signals.md`,
`./Market_Regime.md`, `./Volatility_Regime.md`, `./Market_Research.md` in the
working directory. The coordinator writes the audit trail itself (§7).

`--jev` re-labels regime/sentiment/volatility with the System One model but
requires `TYPESAFE_API_KEY`; in a no-credential scan it is unavailable and the
heuristic labels stand.

## 4. Signals — deterministic strategy evaluation

```bash
$BIN generate-signals --input runs/<run-id>/bars-BTCUSD.json \
  --strategy BollingerBands \
  --history runs/<run-id>/history.json > runs/<run-id>/signals-BTCUSD.json
```

- An empty `data` list is a valid result — it means no setup, not a failure.
- Signals only fire when the condition holds on the **latest candle**.
- If two strategies conflict on the same asset, emit no signal for it and record
  the conflict.
- Every signal from `generate-signals` carries stop_loss/take_profit sizing;
  never hand-craft intents through `generate-trade-intent` for execution without
  adding risk fields.

## 5. Gate — `judge-signals`, a direct tool call, never a subagent

Run the gate through the credential wrapper (from `~/workspace/thales`):

```bash
export PATH="$HOME/.cargo/bin:$PATH"
./scripts/judge-with-jev.sh \
  --input runs/<run-id>/signals-BTCUSD.json \
  --analysis runs/<run-id>/analysis-BTCUSD.json \
  --bars runs/<run-id>/bars-BTCUSD.json \
  --portfolio runs/<run-id>/positions.json \
  --emit report \
  --log runs/<run-id>/audit.md > runs/<run-id>/judged-BTCUSD.json
```

`scripts/judge-with-jev.sh` fetches a short-lived surrogate for the
`custom.typesafe` Secure Vault connector from authd at scan time and exports
it as `TYPESAFE_API_KEY` for exactly one `judge-signals` invocation. The
surrogate is held only in the child process's environment — never written to
disk, never logged, never committed. Verified 2026-09-22: the gate reached
the Jev API through the compiled binary and returned a real adjudicated
verdict (model `jev-1.13.0`).

Rules (non-negotiable):

- `judge-signals` is deterministic tooling with calibrated probabilities. Do not
  put a language model between its verdict and the decision — never delegate it
  to a subagent. An LLM paraphrase of a probability is not the probability.
- It makes **one System One round trip per intent** (4 questions: verdict,
  instrument-quality veto, regime-fit, conviction). Token usage is recorded in
  the report's `usage` field.
- The gate can only **remove or shrink** signals: `skip` drops, `reduce_size`
  scales `size_hint` by `--reduce-factor` (default 0.5). It never creates signals.
- **Never weaken the thresholds mid-run to force a signal through**
  (`--min-probability` 0.55, `--min-confidence` 0.60,
  `--min-instrument-quality` 0.5). If a threshold is wrong, change it in a commit
  with a reason — not inside a run.
- With `--emit report` the output `data` is the full `JudgeReport`
  (`approved`, `rejected`, `verdicts`, `thresholds`, `usage`); without it,
  `data` is only the surviving `TradeIntent` list and feeds `execute-intent`.
- Rejected signals land in `runs/<run-id>/audit.md` via `--log` as
  `| Date/Time | Symbol | Signal Ref | Rejection Reason |` rows.
- **No credential → the gate fails closed.** If the wrapper cannot obtain the
  Jev credential (no authd socket, connector revoked), the run ends at step 5:
  record "gate unavailable — no credential", do NOT execute anything, do not
  bypass. An ungated execution is never acceptable.

## 6. Execute — only what survived the gate, paper only

```bash
# judged.json data is the approved TradeIntent list (--emit intents, the default)
$BIN execute-intent --provider paper --input runs/<run-id>/approved-BTCUSD.json \
  > runs/<run-id>/execution-BTCUSD.json
```

- Validate first (the CLI validates intents before submitting).
- `execute-intent --provider paper` writes to `$PAPER_PORTFOLIO_PATH`; prices
  come from Kraken's **public** Ticker when reachable, else the intent's
  limit/stop price, else a dummy 100.0 with a stderr warning. A dummy-price fill
  is a plumbing test, not a simulation — record which price source was used.
- Never execute an intent that did not come out of `judge-signals`. Never
  re-judge, override, or resize a gated intent; if it looks wrong, return it
  unexecuted with the reason.

## 7. Audit trail — accepted AND rejected, every run

The coordinator appends to `runs/<run-id>/ledger.md` after the run:

```markdown
## Run <run-id> — 2026-09-23 08:00 CT
- Universe scanned: BTCUSD, ETHUSD, SPY
- Positions before: (from positions.json)
- Research sources: <links/citations>
- Data quality: BTCUSD ok (100 bars, latest <ts>); ETHUSD STALE (latest <ts> — skipped, reason)
- Signals generated: 1 (BTCUSD BollingerBands Entry, SL ..., TP ..., rationale)
- Gate verdicts: approved 0, rejected 1 — BTCUSD: instrument-quality 0.42 < 0.50
- Executed: none (nothing survived the gate)
- Outcome: NO TRADE. Reason: ...
```

Accepted candidates get the full row
(`| Date/Time | Asset Class | Symbol | Action | Size/Qty | Entry Price | SL | TP | Max Risk | Signal Ref | Rationale |`);
rejected candidates get
(`| Date/Time | Symbol | Signal Ref | Rejection Reason |`).
`--log` already writes the rejection rows — pass it rather than transcribing.

A run ends. It does not poll, re-open closed work, or re-run a step hoping for a
different answer.

## Honest limits of a no-credential scan

Say these out loud in the report, every time, until the plumbing changes:

1. **The paper provider's market data is synthetic.** `fetch-market-data --provider paper`
   generates 100 sine-wave bars with hardcoded start prices — it is engineered to
   trigger setups, not to reflect any market. A no-credential scan proves the
   pipeline runs; it says nothing about real markets.
2. **Live Kraken public data through the CLI currently requires Kraken credentials**,
   because every kraken command calls `KrakenConfig::from_env()` first. The OHLC and
   Ticker endpoints themselves need no auth — the credential demand is a CLI
   artifact, not an exchange requirement.
3. **`scan-market` for alpaca/paper is a static list**, not a live scan. The only
   live universe scan is Kraken Ticker (credential-gated).
4. **`judge-signals` runs credentialed via `scripts/judge-with-jev.sh`** (wired
   2026-09-22). If the credential is ever unavailable, a run legitimately ends
   at step 5 with "gate unavailable". That is the design working.
5. **Paper fills are approximate**: execution price prefers the live Kraken public
   Ticker, then limit/stop prices, then a dummy 100.0. Synthetic bars + approximate
   fills = plumbing test only.
6. **Nothing here is a live trade.** Paper provider state lives in a local JSON
   file. No order ever leaves the machine without a live provider and Mark's
   explicit per-action word.

## When inputs are missing, stale, or contradictory

Stop and record — never manufacture a signal:

- Empty bar series → skip symbol, log "no data".
- Stale latest bar (> 2x timeframe) → skip symbol, log "stale data".
- Contradictory strategies on one asset → no signal, log the conflict.
- Gate unavailable (no `TYPESAFE_API_KEY`) → end run at step 5, log it.
- Any provider `status: "error"` → stop, record the error, do not retry in a loop.

## Quick reference — JSON contracts

All commands print a `{"status": "ok"|"error", "errors": [], "warnings": [], "data": ...}`
envelope and exit 0 on success, non-zero on validation/provider failure.

- `fetch-market-data` → `data`: `BarSeries {schema_version, bars: [{symbol, market, timeframe, timestamp_unix_ms, open, high, low, close, volume}]}`.
- `normalize-bars` → same `BarSeries`, sorted by `timestamp_unix_ms`.
- `analyze-market --no-report` → `data`: `MarketAnalysis {regime, sentiment, volatility, confidence, recommendation, ...}`.
- `generate-signals` → `data`: `TradeIntent[]`; empty list is a valid no-setup result.
- `judge-signals --emit intents` → `data`: surviving `TradeIntent[]`.
- `judge-signals --emit report` → `data`: `JudgeReport {approved, rejected, verdicts, thresholds, usage}`.
- `execute-intent` → `data`: `ExecutionResult[]` with `provider_order_id`.
- `TradeIntent` carries `intent_id "<market>:<symbol>:<side>:v0"`, `size_hint`,
  `confidence` (replaced by the gate's calibrated probability), `stop_loss`,
  `take_profit`, `order_type`, `time_in_force`, `schema_version: "v0"`.
