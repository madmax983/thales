# Proposed runbook patch — regime detectors integration

> **DRAFT — DO NOT APPLY.** Mark must approve before `docs/morning-scan-runbook.md`
> changes. Prepared 2026-09-29 alongside the `regime` crate
> (`docs/regime-detectors.md`).

## What changes

Two additions, both in the research phase (§3), both advisory:

1. **§3d — reported-regime-events checklist.** The research step already
   searches the web for market context; this adds an explicit checklist item:
   look for newly *reported* ZBT / follow-through-day / market-status calls
   from known technical sources and append them to the append-only log
   `docs/regime-events.jsonl`.
2. **§3e — computed regime reading.** Run the deterministic `regime-status`
   command on fresh **index** bars (SPY — an index, never a candidate) plus the
   events log, and fold the one-line reading into `Market_Regime.md` and the
   `--research` text for `analyze-market`. Advisory evidence only.

Neither change touches the gate-first pipeline (§4–§7): regime context
informs, never decides. No thresholds move.

## Patch

Apply after the end of §3c (the WSB section, which ends with the paragraph
"WSB sentiment is advisory: it informs, never decides. … never fabricated,
never a blocker.") and before `## 4. Signals`:

```markdown
## 3d. Reported regime events — ZBT / FTD / market-status checklist

Zweig Breadth Thrusts are too rare to compute from a feed we don't have
(there is no free NYSE breadth feed, and we never fake breadth data), and
every thrust since WWII made the financial press. So the detection path for
rare thrusts is **reported events, caught by this research step** — the same
way any other market news is caught.

During §3 research, run these searches and append what you find to the
append-only log `docs/regime-events.jsonl` (one JSON object per line —
never edit existing lines):

- IBD Big Picture current market status ("Confirmed Uptrend" / "Uptrend
  Under Pressure" / "Market in Correction" / "Rally Attempt")
- BofA technical strategy notes, Carson Investment Research / Ryan Detrick,
  SentimenTrader — any newly reported Zweig Breadth Thrust or follow-through
  day call

Log line schema:

{"date":"YYYY-MM-DD","event":"ZBT|FTD|MARKET_STATUS","detail":"what was reported","source":"who reported it","url":"link or null","status":"Correction|AttemptedRally|ConfirmedUptrend (MARKET_STATUS only, else null)"}

Rules:

- Append only genuinely new events (same date + event + source = already
  logged, skip it).
- `status` is the source's own normalized call — never paraphrase prose into
  a status you inferred. If the source's wording doesn't match a known
  status, leave `status` null and put the verbatim wording in `detail`.
- Nothing found is a result: record "no new regime events reported" and move
  on. Never fabricate an event to fill the log.

## 3e. Regime reading — `regime-status` (deterministic, advisory)

With the events log current, get the merged regime reading from fresh index
bars (SPY daily — an index, not a candidate):

$BIN regime-status --input runs/<run-id>/bars-SPY.json \
  --events docs/regime-events.jsonl > runs/<run-id>/regime.json

`regime-status` merges the computed FTD state machine (O'Neil CAN SLIM on
the bars) with the reported events log and the curated ZBT history, and
outputs one `RegimeReport`: `regime` (Correction|AttemptedRally|
ConfirmedUptrend|Unknown), `basis` (reported|computed|both), `agreement`,
the FTD evidence, the reported cross-check, and ZBT context.

Fold the headline into the coordinator's own writes:

- One line in `Market_Regime.md` (§8 audit): regime, basis, agreement, ZBT
  context (e.g. "last thrust 2025-04-24, 158d ago").
- The same headline, phrased as observed regime context — never as a
  recommendation — in the `--research` text passed to `analyze-market`.

The regime reading is advisory evidence alongside volatility, WSB, and
Tradytics. It never creates, sizes, approves, or blocks a trade, and it never
lowers a gate threshold. A computed/reported disagreement (`agreement:
false`) is recorded verbatim for a human to resolve — the run does not break
the tie.
```

## Verification after applying

- `grep -n "3d. Reported regime events" docs/morning-scan-runbook.md` shows
  the new section between §3c and §4.
- A dry run: `thales-cli regime-status --input <spy-bars> --events
  docs/regime-events.jsonl` exits 0 with a `REGIME` envelope.
- No changes to §4–§7 (gate-first ordering untouched).
