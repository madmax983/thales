# SPEC-006: Options Strategy Evaluation (vol selling/buying, calendars, iron condors)

**Status**: Draft
**Owner**: Mark
**Created**: 2026-09-23

## 1. User Story
> "As the Thales coordinator, I want the scan to evaluate options-income and
> volatility strategies — not just directional bar setups — so that high-IV
> regimes produce premium-selling candidates and crushed-IV regimes produce
> long-vol candidates, each with defined risk and a real price."

## 2. The "So What?"
- **Problem**: Every strategy in `crates/strategies` is a single-instrument
  directional setup on OHLCV bars. When the scan's own vol forecast (SPEC-005)
  says IV is in the 90th percentile, Thales has no way to *sell* that
  expensiveness — it can only fade or follow price. The richest edge in
  options (IV mean reversion, term-structure shape) is invisible to the
  pipeline.
- **Solution**: An options chain provider (Yahoo, verified 2026-09-23), a
  Black-Scholes pricer crate, a small set of vol-regime evaluators, and a
  multi-leg intent model — phased so each layer is useful before the next
  lands.

## 3. Data — Yahoo options chain (verified live 2026-09-23)

Endpoint: `GET https://query1.finance.yahoo.com/v7/finance/options/{TICKER}`
(`?date={unix}` for one expiration). Unlike the chart API, this one requires
Yahoo's cookie+crumb flow:

1. `GET https://fc.yahoo.com/` with a browser User-Agent, store cookies.
2. `GET https://query1.finance.yahoo.com/v1/test/getcrumb` (same jar) → crumb.
3. `GET /v7/finance/options/SPY?crumb={crumb}` with the jar.

Verified response shape (`optionChain.result[0]`): `quote.regularMarketPrice`,
`expirationDates[]` (SPY: 31), `strikes[]` (SPY: 153), and per expiration
`calls[]`/`puts[]` with `contractSymbol`, `strike`, `bid`, `ask`,
`impliedVolatility`, `openInterest`, `volume`, `inTheMoney`, `expiration`,
`lastTradeDate`. No credential, no key — same posture as the chart provider.

**Provider design** (`crates/providers/yahoo`, additive):
- `fetch_chain(ticker) -> ChainSnapshot` — all expirations, or
  `fetch_chain_expiry(ticker, date)` for one.
- `ChainSnapshot { underlying, quoted_at, expirations: Vec<ExpirySlice> }`,
  `ExpirySlice { expiry_unix, dte, calls: Vec<Quote>, puts: Vec<Quote> }`,
  `Quote { contract_symbol, strike, bid, ask, mid, iv, oi, volume, itm }`.
- Fail closed on crumb rejection (Yahoo rotates crumbs; retry the flow once,
  then error, never fake a chain).
- Fixture-based tests only — no network in `cargo test`.

## 4. Pricer — `crates/options` (new)

Pure, deterministic, no I/O:
- `black_scholes(call|put, spot, strike, t_years, rate, iv) -> (price, delta, gamma, theta, vega)`.
- `implied_vol` available but Yahoo already quotes IV — use theirs, keep ours
  as a cross-check that fails closed on divergence (flags bad quotes).
- Day-count: ACT/365; rate: configurable, default 0.04 with a warning that
  it's an assumption, not a market read.
- Unit tests against textbook values (Hull-style spot checks).

## 5. Evaluators — v1 strategy set

Evaluators consume `(ChainSnapshot, underlying BarSeries, VolForecast)` and
emit candidates only when *all* entry filters pass. Conservative by design:
defined risk first, naked short premium never.

| Evaluator | Regime trigger | Structure | DTE / delta targets |
|---|---|---|---|
| `ShortIronCondor` | IV rank ≥ 70, underlying inside 1σ expected move | short 30Δ put spread + short 30Δ call spread | 30–45 DTE, wings 10Δ |
| `ShortStrangle` | IV rank ≥ 80, high-liquidity underlyings only | short 25Δ put + short 25Δ call | 30–45 DTE |
| `LongStraddle` | IV rank ≤ 20, or pre-event vol crush | long ATM call + long ATM put | 30–60 DTE |
| `CalendarSpread` | term structure steep: back-month IV − front-month IV ≥ 8 pts | short front ATM, long back ATM (call or put) | front 15–30 DTE, back 45–75 DTE |

Shared filters (all four): mid-price only (never bid/ask cross in evaluation);
OI ≥ 500 and volume ≥ 50 per leg (liquidity); max spread width ≤ 10% of mid
(illiquid markets fail closed); earnings within the front expiry → skip short
premium (binary risk); underlying must be in the scan universe.

**IV rank** comes from SPEC-005's vol forecast history: rank today's
front-month ATM IV against the trailing 1y of (chain IV where available,
EWMA forecast σ otherwise). v0 ships IV rank as a *research input* to
`analyze-market` before any evaluator emits candidates.

### Evaluator ↔ Sinclair shortlist mapping (2026-09-23)

Mark's dictated notes from *Positional Option Trading* Part Three drive
*candidate selection* (which tickers/events get evaluated), not the
evaluator math. Confidence level 1 is excluded from Thales entirely.

| Sinclair strategy (confidence) | Thales use | Evaluator served |
|---|---|---|
| Term-structure predictor: contango → short futures, backwardation → long (3) | VIX futures curve read in `analyze-market` research | v2 (futures; no evaluator yet) |
| Fundamental-factor straddles: sell top-quartile / buy bottom-quartile, weekly rebalance; RoE best (~$361/wk, Sharpe ~1.2); long-vol profile = low P/E, low P/CF, high mkt cap, high RoE, high RoA, high D/E (3) | factor-ranked ticker list feeds straddle evaluation | `LongStraddle` / short-premium variant |
| PEAD — drift continues in surprise direction (3) | post-earnings directional candidates stay in the bar-strategy ensemble; straddle side via factor screen | ensemble + `LongStraddle` |
| Earnings IV run-up/collapse (2) | earnings calendar in research; short front vol into event, calendars across it | `ShortStrangle`, `CalendarSpread` |
| Overnight effect — index VRP realized overnight; sell very short-dated spanning overnights (2) | 0–7 DTE index short-premium sleeve | `ShortStrangle` (short-dated variant) |
| FOMC — buy ES two days before release (2) | FOMC calendar in research; directional | ensemble (event-aware) |
| Weekend effect — theta doesn't stop (2) | short premium over weekends | `ShortIronCondor` / `ShortStrangle` |
| VVIX extremes — buy/sell VIX futures, hedge with SPY straddles (2) | VVIX read in research | v2 |

Sizing for all of the above follows the skill's Part 2: edge gap
(forecast − implied) vs. forecast uncertainty, with portfolio-level
concentration limits on short-vol (all VRP strategies are short the same
crash risk).

## 6. Intent model — multi-leg

Current `TradeIntent` is single-leg directional. Options candidates need:

```rust
pub struct Leg {
    pub contract_symbol: String, // OCC, e.g. SPY260930C00768000
    pub side: LegSide,           // Buy / Sell
    pub quantity: u32,
    pub limit_price: f64,        // mid at evaluation; paper fills at mid
    pub greeks: Greeks,          // evaluated snapshot
}
pub struct MultiLegIntent {
    pub intent_id: String,       // options:<strat>:<underlying>:<expiry>:<hash>
    pub underlying: String,
    pub legs: Vec<Leg>,
    pub net_debit_or_credit: f64,
    pub max_profit: f64,
    pub max_loss: f64,           // defined risk, always computed
    pub breakevens: Vec<f64>,
    pub dte: u32,
    pub iv_rank_at_entry: f64,
}
```

`intent_id` includes a hash of (legs, expiry) so two evaluators can't collide.
`validate-intent` learns the multi-leg shape; `execute-intent` stays
paper-only until Mark authorizes otherwise per action (standing rule).

## 7. Gate implications

`judge-signals`' four questions assume directional setups. For v1, options
candidates go through the gate with two adapted questions: instrument-quality
veto (liquidity filters are the veto inputs) and a vol-regime-fit check
("does selling premium fit an IV-rank-85 regime?"). Conviction/verdict
calibration for multi-leg is v2 work — v1 candidates are research-grade
(paper, advisory) even if they pass.

## 8. Phased build plan

- **v0** — chain provider + fixtures, `crates/options` pricer, IV rank as
  `analyze-market` research input. No candidates emitted. (Useful alone:
  the scan finally *sees* expensiveness.)
  **Status 2026-09-23: BUILT** — `yahoo-provider::chain` (cookie+crumb
  flow, `ChainSnapshot`/`ExpirySlice`/`OptionQuote`, fixture tests),
  `crates/options` (Black-Scholes + Greeks, `iv_rank`/`iv_percentile`,
  `edge` module), `fetch-options-chain` CLI (front ATM IV + optional
  rank/percentile from `--iv-history`), runbook §2d. No candidates
  emitted, as specified.
- **v1** — the four evaluators emitting `MultiLegIntent`s in paper mode,
  liquidity/earnings filters, runbook §4b (options candidates alongside the
  bar-strategy ensemble, never instead of it). Candidate selection driven
  by the Sinclair shortlist (§5 mapping table).
- **v2** — gate adaptation for multi-leg, exit/management rules (50%-of-max
  profit take, 2×-credit stop on short premium), roll logic.

**Sinclair framework** (2026-09-23, Mark's direction): the volatility
estimators (Parkinson, Garman–Klass, Rogers–Satchell, Yang–Zhang) and
forecast combining live in `crates/volatility::ohlc`; edge/VRP/hedged-P&L
in `crates/options::edge`; the full framework write-up in
`~/workspace/skills/sinclair/SKILL.md`. Mark's confidence-rated strategy
shortlist (level 3: term-structure predictor, factor straddles, PEAD;
level 2: earnings IV, overnight effect, FOMC, weekend effect, VVIX
extremes; level 1 excluded) drives v1 candidate selection.

## 9. Honest limits

- Yahoo quotes are delayed ~15 min. **Accepted 2026-09-23 (Mark): fine for
  swing setups — Thales is not day trading, so no real-time quote source is
  needed.** Evaluation is regime work, not scalping.
- No futures options on Yahoo — ES/NQ vol plays need another source (TBD).
- Paper fills at mid are generous; real short-premium fills are worse.
- American exercise / assignment risk is not modeled in v0–v1 (avoid
  ITM short legs near ex-div; documented, not priced).
- Earnings binary risk is filtered by date, not modeled.
- IV rank needs a year of history; before the chain cache accumulates,
  rank falls back to EWMA forecast σ with a warning.
- This spec's endpoint findings are from 2026-09-23; Yahoo changes shapes
  without notice — the provider must fail closed on schema drift.
