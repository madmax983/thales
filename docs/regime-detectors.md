# Regime Detectors — ZBT (reported) + FTD (computed)

Status: implemented 2026-09-29. Advisory only — see "What this is not" below.

## What this is

Two regime-*change* detectors that answer **"has the market regime changed?"**
as a `RegimeState` reading (`Correction` | `AttemptedRally` |
`ConfirmedUptrend` | `Unknown`). They are the coordinator's market-weather
input: they feed `Market_Regime.md` and `analyze-market --research`, the same
way WSB sentiment and Tradytics evidence do.

## What this is not

- **Not a strategy.** No `TradeIntent` is ever emitted, no confidence is
  scored, nothing here can clear the `judge-signals` gate or reach execution.
  `judge-signals` remains the only route toward execution.
- **Not a threshold-weakener.** A bullish regime reading never lowers a gate
  threshold or manufactures a candidate. Paper only, as always.
- **Not a prediction.** A regime reading describes the tape and the published
  technical consensus; it does not forecast.

## Design: reported events are the authority for rare thrusts

The original plan was to compute the Zweig Breadth Thrust from raw
NYSE advance/decline data. That plan was dropped for a structural reason:
**there is no free, reliable NYSE breadth feed.** FRED carries advance/decline
series (`UPADNS`/`DOWADNS`) but requires an API key and is a research database,
not a scan-time feed; every other source is paywalled or scrape-only. Building
a math engine on unfetchable data would have meant fabricating breadth — worse
than useless.

So the design inverts:

| Detector | Primary path | Cross-check |
|---|---|---|
| **ZBT** | Reported events (curated history + events log) | — (no breadth feed exists) |
| **FTD regime** | Computed state machine (Yahoo daily bars + volume, deterministic, paywall-free) | IBD's published market status, via the events log |

A ZBT is a ~1-in-4-years event that every technical desk writes up (roughly 20
since WWII per Carson/Detrick/Ned Davis counts, which vary slightly by
methodology). Reported events *are* the detection path for rare thrusts: the
scan's research step catches them the way it catches any other market news.

**Residual gap, stated honestly:** a thrust with zero media coverage would be
missed. Historically implausible for an event this rare and this watched, but
not impossible — so it is recorded here instead of hand-waved.

## Components

### 1. Curated ZBT history — `crates/regime/data/zbt_history.json`

Source-attributed thrust dates, embedded in the `regime` crate via
`include_str!`. Each entry carries `date`, `date_precision` (`day`/`month`/
`year`), `source`, `url`, and `note`. Verified day-precision entries:

- `2025-04-24` — Carson Investment Research / Ryan Detrick (20th since WWII)
- `2023-11-03` — Ned Davis Research (18th since 1945; 10-day window
  2023-10-23…2023-11-03, indicator 0.36 → 0.62)
- `2023-03-31` — Humble Student of the Markets (rare 12-month cluster with Nov 2023)

Plus month-precision textbook examples (Oct 1966, Aug 1982, Mar 2009) from
Zweig's literature. `days_since_thrust` refuses to compute from imprecise
dates — no fabricated numbers.

### 2. Reported-events log — `docs/regime-events.jsonl`

Append-only, one JSON object per line, maintained by the scan's research step:

```json
{"date":"2025-04-24","event":"ZBT","detail":"Zweig Breadth Thrust triggered","source":"Carson Investment Research / Ryan Detrick","url":"https://…","status":null}
```

- `event`: `ZBT` | `FTD` | `MARKET_STATUS`.
- `status` (optional, for `MARKET_STATUS`): normalized regime —
  `Correction` / `AttemptedRally` / `ConfirmedUptrend`. IBD's "Uptrend Under
  Pressure" maps to `ConfirmedUptrend` with `weakening: true`.
- **Never edit in place; append only.** A missing file reads as an empty log
  (not an error); a malformed line fails closed.

### 3. Computed FTD state machine — `crates/regime/src/ftd.rs`

O'Neil CAN SLIM, deterministic, over trailing daily index bars (default 252):

- **Day 1**: a bar closes up where the trailing-20-bar low was set within the
  last 3 bars → `AttemptedRally`.
- **Follow-through**: rally days 3–10 (classic window 4–7, accepted 3–10),
  close-to-close gain ≥ 1% on volume greater than the prior day →
  `ConfirmedUptrend`.
- **Failure**: any attempt bar undercuts the Day-1 low → `Correction`; no
  follow-through by end of rally day 10 → `Correction` ("stalled").
- **Uptrend invalidation (v1)**: undercutting the follow-through bar's low →
  `Correction`.
- Empty input → `Unknown` with a note (a result, not a failure). Non-finite or
  invalid OHLCV → typed error (fail closed).

**v2 TODO:** distribution-day counting (O'Neil's institutional-selling half:
5+ distribution days in a window ends the uptrend). `FtdDetails.distribution_days`
is stubbed `None` today; the confirmation logic is structured so the counter
slots in without changes.

### 4. Merge — one `regime-status` command

```bash
thales-cli regime-status --input <bars-json> [--events docs/regime-events.jsonl] [--zbt-history <path>]
```

Reads the events log, runs FTD on the bars, merges into one `RegimeReport`
(the coordinator's unit of consumption):

```json
{
  "status": "ok", "errors": [], "warnings": [],
  "data": {
    "detector": "REGIME",
    "regime": "Correction|AttemptedRally|ConfirmedUptrend|Unknown",
    "as_of": "YYYY-MM-DD",
    "basis": "reported|computed|both",
    "ftd": { "detector": "FTD", "state": "…", "as_of": "…", "symbol": "…",
             "details": { "rally_day": 5, "day1_date": "…", "day1_low": 451.09,
                          "follow_through_date": "…", "follow_through_gain_pct": 0.015,
                          "follow_through_volume_ratio": 2.0,
                          "follow_through_low": 461.07,
                          "distribution_days": null, "notes": [] } },
    "reported": { "market_status": { "date": "…", "detail": "…", "source": "IBD Big Picture",
                                     "url": null, "mapped": "ConfirmedUptrend",
                                     "weakening": false },
                  "reported_ftd": null, "events_seen": 3 },
    "zbt": { "last_thrust": { "date": "2025-04-24", … },
             "days_since_thrust": 286, "within_90d_window": false, "history_count": 6 },
    "agreement": true,
    "notes": []
  }
}
```

**Merge rule** (deterministic, in `crates/regime/src/merge.rs`):

1. Computed FTD is the primary reading.
2. Latest reported `MARKET_STATUS` is the cross-check: agreement →
   `basis: both`; disagreement → hold computed, `basis: computed`,
   `agreement: false`, disagreement spelled out in `notes` for a human to
   resolve.
3. Computed `Unknown` + reported present → reported stands,
   `basis: reported`.
4. ZBT is **context, not a state**: last thrust date, days-since, source. A
   thrust inside the 90-day post-thrust window is flagged in `notes` —
   context only, never a signal.

## Coordinator consumption

The coordinator runs `regime-status` on fresh **index** bars (SPY/DIA/QQQ —
an index, not a candidate) plus the events log, and:

- folds the one-line reading into `Market_Regime.md` (its own audit write,
  §8 of the runbook);
- folds the headline (`regime`, `basis`, `agreement`, ZBT context) into the
  `--research` text passed to `analyze-market`, phrased as observed regime
  context — never as a recommendation.

The regime reading is advisory evidence alongside volatility, WSB, and
Tradytics. It never creates, sizes, or approves a trade.

## Tests

`crates/regime` unit tests (deterministic, synthetic):

- FTD confirmation (decline → Day 1 → +1.5% on 2× volume, rally day 5).
- FTD undercut → `Correction`.
- Follow-through-strength bar on rally day 2 (outside the window) → not
  confirmed; stalled by day 10 → `Correction`.
- Confirmed uptrend invalidated on follow-through-low undercut.
- Empty input → `Unknown` with note; NaN close → typed error (fail closed).
- ZBT history: parse, newest-first, last-thrust lookup, days-since on day
  precision, refusal on month precision.
- Events log: valid parse, latest-status selection, missing file → empty,
  malformed line → error.
- Merge: agreement → `both`; disagreement → hold computed + flag; no events
  → computed only; empty bars + reported → reported.

## Files

- `crates/regime/` — the crate (`ftd.rs`, `zbt.rs`, `events.rs`, `merge.rs`,
  `data/zbt_history.json`)
- `crates/cli/src/main.rs` — `regime-status` command
- `docs/regime-events.jsonl` — the append-only reported-events log
- `docs/regime-detectors-runbook-patch.md` — proposed runbook integration
  (**draft — Mark must approve before `docs/morning-scan-runbook.md` changes**)
