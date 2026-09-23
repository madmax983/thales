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
| `fetch-market-data --provider paper` | none | works (SYNTHETIC bars — pipeline tests only, see §Honest limits) |
| `fetch-market-data --provider yahoo` | none | works — REAL bars (unofficial Yahoo chart API, no SLA; gentle use: one pass per scan) |
| `fetch-market-data --provider kraken` | `KRAKEN_API_KEY`, `KRAKEN_API_SECRET` | fails — even though OHLC is a public endpoint, the CLI demands credentials first |
| `fetch-market-data --provider alpaca` | `ALPACA_API_KEY`, `ALPACA_API_SECRET`, `ALPACA_BASE_URL` | fails (`ALPACA_BASE_URL` has no default) |
| `scan-market --provider kraken` | `KRAKEN_API_KEY`, `KRAKEN_API_SECRET` | fails (Ticker is public; the CLI still gates on credentials) |
| `scan-market --provider alpaca` | none | returns a STATIC watchlist, not a live scan |
| `scan-market --provider paper` | none | serves the audited 124-symbol universe manifest; **fails closed** if unreadable |
| `judge-signals` | `TYPESAFE_API_KEY` | **fails closed** — the run stops; signals never pass ungated |
| `analyze-market --jev` | `TYPESAFE_API_KEY` | heuristic labels only without `--jev` |

## 1. State — what do we hold, what is worth looking at?

```bash
mkdir -p runs/<run-id>
$BIN get-positions --provider paper > runs/<run-id>/positions.json
$BIN scan-market --provider paper --top-n 10 > runs/<run-id>/universe.json   # audited manifest (124 symbols); fails closed if unreadable; see §Honest limits
```

Check `positions.json`: envelope is `{"status","errors","warnings","data": [...]}`.
If status is `error`, stop the run and record the error in the ledger.

## 2. Data — fetch and normalize, one candidate at a time

For each candidate symbol from the universe (a bounded shortlist — at most
`--top-n`, never chase; the full 124-symbol cheap ranking is still future work):

```bash
$BIN fetch-market-data --provider yahoo --symbol SPY --timeframe 1d > runs/<run-id>/fetch-SPY.json
$BIN normalize-bars --input runs/<run-id>/fetch-SPY.json > runs/<run-id>/bars-SPY.json
```

`--symbol` takes the entry's `yahoo` alias from `crates/cli/universe/universe.json`
(e.g. `SPX` → `^GSPC`, `ES` → `ES=F`, `BTCUSD` → `BTC-USD`); US equities/ETFs pass
through unchanged. `--provider yahoo` is the real-data route (no credential).
`--provider paper` still exists for pipeline tests but generates synthetic
sine-wave bars — never use it for a real scan.

Verify `bars-<SYM>.json` has a non-empty `data.bars` array sorted by
`timestamp_unix_ms`. If the series is empty or the latest bar is stale
(older than 2x the timeframe), **stop for that symbol and record why**.
Never fabricate bars.

## 2b. Volatility forecast — one deep fetch per shortlist symbol

The default 6-month Yahoo window is too short for volatility estimation.
For each shortlist symbol (and only the shortlist — one gentle pass per
scan), fetch deep history once, normalize, and forecast:

```bash
$BIN fetch-market-data --provider yahoo --symbol SPY --timeframe 1d --history 2y > runs/<run-id>/fetch-SPY-deep.json
$BIN normalize-bars --input runs/<run-id>/fetch-SPY-deep.json > runs/<run-id>/bars-SPY-deep.json
$BIN forecast-volatility --input runs/<run-id>/bars-SPY-deep.json > runs/<run-id>/vol-SPY.json
```

`--history` is yahoo-only (`1y|2y|5y|10y|max`; `1h` capped at `2y`) and fails
closed otherwise. `forecast-volatility` fits EWMA (RiskMetrics λ=0.94) over
log returns of `adjusted_close` when every bar carries one, else `close`
with a warning. It needs ≥ 60 returns and fails closed on anything less,
on non-finite input, or on `--estimator garch11` (specified, not built yet).

Fold the result into `analyze-market --research`: daily σ, annualized σ,
persistence, half-life, and any warnings. A volatility forecast is
**evidence for sizing and risk, never a trade signal** — it cannot clear
the Jev gate on its own.

## 2d. Options-chain snapshot — v0 research input (SPEC-006)

For index/equity candidates, pull one Yahoo options-chain snapshot per
symbol and fold the IV read into `analyze-market --research`. This is the
scan finally *seeing* expensiveness. v0 emits no options candidates —
the chain is context, not a signal.

```bash
$BIN fetch-options-chain --symbol SPY --min-dte 7 \
  --iv-history runs/iv-history/SPY.json \
  > runs/<run-id>/chain-SPY.json
```

- `--min-dte 7` skips 0DTE noise for the front-ATM-IV read.
- `--iv-history` is a JSON array of past front-ATM IV readings (one per
  day, persisted by the scan). Without it the read ships without
  rank/percentile — honest, not fabricated.
- Record in `--research`: spot, front DTE, front ATM IV, IV rank and
  percentile when available (e.g. "SPY front (7 DTE) ATM IV 18.2%,
  rank 82/percentile 91 — expensive vs trailing year").
- Chain auth (cookie/crumb) failing closed → skip the symbol's IV read,
  note it in the audit trail, continue the scan. Never fake the read.
- The Sinclair event checklist lives in research too: earnings dates,
  FOMC dates, VVIX extremes (see `~/workspace/skills/sinclair/`).

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

## 3b. Tradytics evidence — options context for each candidate

Do this as part of §3, before running `analyze-market`. For each candidate
on the cheap-screen shortlist (at most `--top-n` — never chase), pull
Tradytics options context as evidence:

```bash
~/workspace/skills/tradytics/bin/fetch-with-cookie.sh gex BTCUSD > runs/<run-id>/tradytics-BTCUSD-gex.json
# repeat for dealer, summary, candles as needed — one ticker at a time, ≥2s apart
```

- Preferred route: `fetch-with-cookie.sh` (whitelisted read-only datasets;
  session cookie via Secure Vault surrogate — never exposed). See the
  Tradytics skill's `references/api-helper.md` for the dataset list, pacing,
  and failure taxonomy.
- If the script reports the session expired (exit 3), the cookie must be
  resubmitted via the secure card; fall back to the DOM-reading browser task
  per the skill. The darkpool ticker has no JSON API — use the browser task
  if dark-pool prints are needed.
- Fold the headline numbers (GEX levels, dealer deltas, sentiment, notable
  flow) into the `--research` text passed to `analyze-market` above, so the
  judge sees them as evidence. Keep the raw JSON under `runs/<run-id>/`.
- Tradytics evidence is advisory: it informs, never decides. Missing or
  stale Tradytics data is recorded ("Tradytics unavailable: <reason>") and
  the run continues on web research + Thales output — it never blocks the
  gate and is never fabricated.

## 3c. WSB sentiment — retail-mania context for each candidate

Do this as part of §3, before running `analyze-market`. For each candidate
on the cheap-screen shortlist (at most `--top-n` — never chase), pull a
WallStreetBets sentiment read per `~/workspace/skills/wsb/SKILL.md`:

1. Run the skill's 2–3 `browser.search` queries per ticker
   (`ranking_intent: "engagement"`); save the raw results verbatim as
   `runs/<run-id>/wsb-<SYMBOL>-raw.json`.
2. Score deterministically with the skill's keyword scorer:
   ```bash
   ~/workspace/skills/wsb/bin/score-wsb.py \
     --input runs/<run-id>/wsb-<SYMBOL>-raw.json \
     --symbol <SYMBOL> > runs/<run-id>/wsb-<SYMBOL>-sentiment.json
   ```
   Exit 0 = scored (even when `quiet` — that is a result); exit 2 =
   invalid input file. This is the floor: it always runs, costs nothing,
   and works when Jev is down.
3. Score the same raw file with one Jev turn (Noul + Score primitives —
   directional lean is categorical, so it goes through a choice question):
   ```bash
   ~/workspace/skills/wsb/bin/score-wsb-jev.py \
     --input runs/<run-id>/wsb-<SYMBOL>-raw.json \
     --output runs/<run-id>/wsb-<SYMBOL>-jev.json
   ```
   Four questions in one round trip: `lean` (choice: bullish/bearish/
   mixed/quiet, with probabilities), `conviction` (score 0–4: attention
   intensity), `euphoria` (noul: crowded-trade check), `surface` (noul:
   worth a line in the brief?). If the turn fails, the run continues on
   the deterministic score and records "Jev scorer unavailable: <reason>".
4. Fold both headlines into the `--research` text passed to
   `analyze-market`, phrased as observed chatter — never as a
   recommendation: keyword `lean` + `mention_volume`, Jev lean + its
   probability mass, `conviction`, `euphoria` vs `euphoria_flag`, and
   `surface`. When the two leans disagree, say so explicitly ("keyword
   scorer mixed, Jev bullish 0.62 / mixed 0.38") — the disagreement is
   data for the judge, not a tie to break by hand.

Read it contrarian: euphoria (either scorer's flag) marks a **crowded
trade**, not confirmation. Futures (ES/NQ), indices, and most ETFs usually
score `quiet` — WSB talks single stocks and 0DTE gambles; `quiet` is a
result, not a failure.

WSB sentiment is advisory: it informs, never decides. Missing or empty
results are recorded ("WSB unavailable: <reason>") and the run continues on
web research + Thales output — never fabricated, never a blocker.

## 4. Signals — deterministic strategy evaluation

One setup is not enough to read a market. Evaluate a small ensemble per
symbol — each strategy fires independently, and the gate judges each
candidate on its own. The ensemble (regime coverage in parentheses):

- `BollingerBands` (mean reversion)
- `RsiMeanReversion` (mean reversion)
- `Supertrend` (trend following)
- `DonchianBreakout` (breakout)

```bash
for STRAT in BollingerBands RsiMeanReversion Supertrend DonchianBreakout; do
  $BIN generate-signals --input runs/<run-id>/bars-BTCUSD.json \
    --strategy $STRAT \
    --history runs/<run-id>/history.json > runs/<run-id>/signals-BTCUSD-$STRAT.json
done
```

- An empty `data` list is a valid result — it means no setup, not a failure.
- Signals only fire when the condition holds on the **latest candle**.
- **Strategy identity in `intent_id`:** every intent id carries its strategy
  (`<market>:<symbol>:<strategy>:<side>:<timestamp_ms>`), so two ensemble
  strategies firing on the same symbol/side can never collapse into one id.
- **Ensemble dedup — at most one position per symbol.** After the four
  strategy files are written, resolve them deterministically with
  `dedupe-signals` before anything reaches the gate:

```bash
$BIN dedupe-signals \
  --input runs/<run-id>/signals-BTCUSD-BollingerBands.json \
  --input runs/<run-id>/signals-BTCUSD-RsiMeanReversion.json \
  --input runs/<run-id>/signals-BTCUSD-Supertrend.json \
  --input runs/<run-id>/signals-BTCUSD-DonchianBreakout.json \
  --log runs/<run-id>/audit.md \
  --report runs/<run-id>/dedup-BTCUSD.json \
  > runs/<run-id>/deduped-BTCUSD.json
```

- **Conflict rule:** if two strategies fire opposite directions on the same
  symbol (any `buy` and any `sell` across its strategy files), `dedupe-signals`
  emits nothing for it — the conflict is recorded in the audit trail and
  neither side is gated.
- **Same-direction corroboration:** multiple same-side candidates collapse to
  exactly one intent — highest confidence wins; ties break on strategy name,
  then intent_id. Corroboration never multiplies exposure: the dropped
  candidates are recorded in the disposition report and the audit trail, and
  are never gated or executed. The kept intent is not modified — its
  confidence is not inflated for being corroborated.
- **The four raw strategy files are kept** in the run directory — never
  deleted, never overwritten. An empty `data` list in one of them is that
  strategy's no-signal record.
- Every signal from `generate-signals` carries stop_loss/take_profit sizing;
  never hand-craft intents through `generate-trade-intent` for execution without
  adding risk fields.

## 5. Gate — `judge-signals`, a direct tool call, never a subagent

Run the gate through the credential wrapper (from `~/workspace/thales`).
The input is the **deduped** file from step 4 — at most one intent per symbol,
already conflict-checked. Gate it directly; there is no second conflict check
here:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
# Deduped file holds at most one intent per symbol (empty on conflict or
# no-signal). Empty files are valid no-setups: skip them without burning a
# Jev call.
F=runs/<run-id>/deduped-BTCUSD.json
if python3 -c "import json,sys; sys.exit(0 if (json.load(open('$F')).get('data')) else 1)"; then
  ./scripts/judge-with-jev.sh \
    --input $F \
    --analysis runs/<run-id>/analysis-BTCUSD.json \
    --bars runs/<run-id>/bars-BTCUSD.json \
    --portfolio runs/<run-id>/positions.json \
    --emit report \
    --log runs/<run-id>/audit.md > runs/<run-id>/judged-$(basename $F .json).json
else
  echo "$(date -u +%FT%TZ) | BTCUSD | no deduped candidate — nothing to gate" >> runs/<run-id>/audit.md
fi
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
- Universe scanned: <symbols from universe.json, first N of the manifest>
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

### Ensemble audit requirements

Every run preserves, per symbol:

- **Symbol and strategy** — which of the four strategies fired, and which one
  survived the dedup (kept strategy + intent_id, e.g.
  `crypto:BTCUSD:BollingerBands:buy:1790183127000`).
- **Signal / no-signal** — all four raw strategy files
  (`signals-<SYM>-<STRAT>.json`) stay in the run directory; an empty `data`
  list in one of them is that strategy's no-signal record. Never delete or
  overwrite them.
- **Gate verdict and reasons** — `judge-signals --emit report --log audit.md`
  records the verdict, instrument-quality, regime fit, conviction, and reasons
  for every deduped candidate.
- **Conflict / dedup disposition** — `dedupe-signals --log audit.md --report
  dedup-<SYM>.json` records, per symbol, `single` / `deduped` / `conflict` /
  no-signal; which intent was kept; which candidates were dropped as
  corroborators; and which were cancelled by conflict (never gated).
- **All four raw strategy files** — retained verbatim as the audit record of
  what each strategy saw on the latest candle.

A run ends. It does not poll, re-open closed work, or re-run a step hoping for a
different answer.

## Honest limits of a no-credential scan

Say these out loud in the report, every time, until the plumbing changes:

1. **Real bars now come from the Yahoo provider.** `fetch-market-data --provider yahoo`
   hits Yahoo Finance's public chart API (no key, no signup) and returns real
   OHLCV for stocks, ETFs, indices (`^GSPC`), futures (`ES=F`), and crypto
   (`BTC-USD`) — verified live 2026-09-23 (SPY/QQQ 127 daily bars, ^VIX 129,
   ES=F 128, BTC-USD 185). The `paper` provider's synthetic sine-wave bars
   remain for pipeline tests only; never use them for a real scan. Caveats:
   Yahoo's API is unofficial (no SLA), so use it gently — one pass per scan —
   and the full 124-symbol cheap ranking is still future work: scans run on a
   bounded shortlist only.
2. **Live Kraken public data through the CLI currently requires Kraken credentials**,
   because every kraken command calls `KrakenConfig::from_env()` first. The OHLC and
   Ticker endpoints themselves need no auth — the credential demand is a CLI
   artifact, not an exchange requirement.
3. **`scan-market --provider paper` serves the audited universe manifest**
   (`crates/cli/universe/universe.json`, embedded at compile time): 104
   Moontower coverage tickers + Mark's 23 symbols (FB recorded as META) +
   BTCUSD/ETHUSD + ES/NQ futures = 124 canonical symbols with asset class,
   provider aliases, and provenance. Entry order is scan priority;
   `--top-n` takes the first N. If the manifest is unreadable or invalid,
   the command exits non-zero with `status: error` — it never silently
   falls back to a short list. The alpaca arm is still a static watchlist;
   the only live universe scan is Kraken Ticker (credential-gated).
4. **`judge-signals` runs credentialed via `scripts/judge-with-jev.sh`** (wired
   2026-09-22). If the credential is ever unavailable, a run legitimately ends
   at step 5 with "gate unavailable". That is the design working.
5. **Paper fills are approximate**: execution price prefers the live Kraken public
   Ticker, then limit/stop prices, then a dummy 100.0. Approximate fills on
   real bars = realistic paper plumbing, but still plumbing.
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
- `TradeIntent` carries `intent_id "<market>:<symbol>:<strategy>:<side>:<timestamp_ms>"`
  (strategy is part of the id so ensemble strategies cannot collide), `size_hint`,
  `confidence` (replaced by the gate's calibrated probability), `stop_loss`,
  `take_profit`, `order_type`, `time_in_force`, `schema_version: "v0"`.
