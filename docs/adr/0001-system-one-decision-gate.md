# ADR 0001: A System One decision gate between signals and execution

- **Status**: Accepted
- **Date**: 2026-09-21

## Context

Thales generates signals mechanically. A strategy computes an indicator, the
indicator crosses a level, and a `TradeIntent` comes out the other side. That is
the right way to generate candidates, but it is a poor way to decide what to
trade, for two reasons.

**A strategy cannot see the instrument.** `RsiMeanReversion` produces the same
textbook signal on a major pair and on a thin novelty token whose price action is
a handful of participants and a bot. The numbers look identical; only something
that knows what the symbol *is* can tell them apart. The repository still carries
`apenft_bars.json` and `koban_bars.json` from a prior autonomous run that traded
exactly these, which is the concrete failure this ADR responds to.

**A strategy cannot see the regime.** A mean-reversion strategy will fire happily
into a strong trend, and its `confidence` field says nothing useful: it is a
mechanical byproduct, not a calibrated probability, so nothing downstream can
distinguish a marginal setup from an obvious one.

The obvious fix is to put a judgement step between generation and execution. An
LLM can do this, but the economics are wrong for a loop that runs continuously
over many candidates: seconds of latency and per-token output costs for what is,
in the end, a three-way classification.

## Decision

Use TypeSafe AI's System One API (model **Jev**) as the judgement layer.

A System One model is not autoregressive. It evaluates *typed questions* against
a *state* and returns the answers directly, each with a probability distribution
and a confidence score. That shape fits this problem exactly:

- the output is a decision, not prose that has to be parsed;
- the probabilities are calibrated, so thresholds mean something;
- output tokens are free and input is $0.042/MTok, so gating every candidate is
  affordable;
- latency is ~0.1s rather than ~8s, so the gate does not dominate the loop.

Two integration points, sharing one client crate (`jev-provider`):

1. **`thales-cli judge-signals`** — the gate proper. Builds a state from the
   proposed trade, the market analysis and summarised recent price action, then
   asks four questions in one round trip: a three-way verdict
   (`execute`/`reduce_size`/`skip`), an instrument-quality veto, a regime-fit
   check and a conviction rating.

2. **`thales-cli analyze-market --jev`** — replaces the heuristic regime,
   sentiment and volatility labels with classified ones, keeping the probability
   mass behind each. The heuristic reading is passed in as part of the state, so
   the model refines a reading rather than starting from nothing.

### The gate rules

All judgement lives in the model's probabilities. The code only thresholds them,
so every rejection is explained by one rule and one number:

1. the instrument clears `min_instrument_quality` (default 0.5) — a standing veto;
2. the model's confidence in its own verdict is at least `min_confidence` (0.60);
3. the verdict is not `skip`;
4. the probability of the chosen action is at least `min_probability` (0.55).

On approval, `confidence` is replaced by the calibrated probability of `execute`,
and a `reduce_size` verdict scales `size_hint` by `reduce_factor` (0.5).

## Consequences

**The gate cannot create signals, only remove or shrink them.** It is a filter,
not a strategy. A missing `TYPESAFE_API_KEY` fails the command loudly rather than
passing signals through ungated, so the gate cannot silently become a no-op.

**`judge-signals` emits `TradeIntent`s by default**, so it drops into an existing
pipeline between `generate-signals` and `execute-intent` without changing either.
`--emit report` returns the full audit trail instead.

**Every verdict is auditable.** The reasoning is appended to the intent's
`rationale`, so it survives into the execution record, and `--log` appends
rejections to `portfolio.md` in the columns `AGENTS.md` already specifies.

**A new external dependency sits in the trading path.** It is contained: the
client is one crate with typed questions validated against the API's documented
limits before anything is sent, 429/529 retried with exponential backoff, and
every failure mode mapped to a distinct error. The gate is opt-in per command.

**Bars are summarised, never embedded.** Jev's state window is finite, so
`PriceSummary` reduces an arbitrarily long series to a fixed set of statistics.

## Alternatives considered

- **An LLM behind the same interface.** Same judgement, ~100x the cost and ~75x
  the latency, plus prose that has to be parsed and uncalibrated confidence. The
  `jev-provider` seam means this could be swapped in later if needed.
- **More heuristics (a liquidity filter, a regime-compatibility matrix).** Cheap
  and deterministic, but it is an endless list of special cases, and the
  instrument-quality question is exactly the kind of judgement that does not
  reduce to a threshold on volume.
- **Folding the judgement into each strategy.** Would duplicate it across 80+
  strategies and conflate "did this fire?" with "should we act on it?".
