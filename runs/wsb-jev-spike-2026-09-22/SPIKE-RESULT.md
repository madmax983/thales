# WSB Jev-scorer spike — NVDA, 2026-09-22 night

## What this was
Mark asked whether WSB sentiment scoring is a good use case for Jev's Noul
and Score question types. This spike ran both scorers on the SAME input and
compared. One Jev turn, four questions, no runbook/cron changes.

Note: the original NVDA raw file from the morning live test did not survive
(it was never written to a run dir), so the spike used fresh searches from
tonight — 8 results across 3 queries. The keyword-vs-Jev comparison below is
on identical input, which is what matters.

## Inputs
- `wsb-NVDA-raw.json` — 8 results: ApeWisdom (119 mentions +183%, 54% sentiment,
  87 WSB mentions/24h by 55 users, 54% positive vs 46% negative), AltIndex
  (349 mentions/day +29%, 59/100 neutral), Benzinga via TradingView (stale:
  "Redditors remain bullish"), CryptoBriefing (NVDA tops finance-Reddit
  discussion in 2026), 2x ainvest options-signal pieces ("cautious
  accumulation", "confidence, not euphoria", put/call 0.80–0.88), 1x stale
  ainvest WSB-rally piece, 1x GitHub factor report (NVDA not in mania top 3).
- `jev-spike.py` — builds state + 4 questions, calls jev.py, writes the full
  turn to `wsb-NVDA-jev.json`.

## Results

### Deterministic keyword scorer (`wsb-NVDA-sentiment.json`)
- lean: **mixed** (bullish 5, bearish 3, mixed 2, net_score 2)
- mention_volume: **high** (driven by result_count = 8 — really "search
  returned plenty", not true mention intensity)
- euphoria_flag: **false**

### Jev turn (`wsb-NVDA-jev.json`) — 2.8s, 2244 in / 99 out tokens
- lean (choice): **bullish**, p=0.62 (mixed 0.38, bearish 0.0, quiet 0.0),
  confidence 0.49
- conviction (score): **2.53** — straddles "moderate buzz" (0.46) and
  "heavy buzz" (0.53); 0.0 on "euphoric pile-on", confidence 0.6
- euphoria (noul): **0.2** — not euphoric, agrees with keyword flag
- surface (noul): **0.54** — just over the 0.50 gate: earns one line in the
  brief, barely

## Read
- The lean disagreement is the interesting bit: keywords saw "puts",
  "caution", "hedging" and said mixed; Jev read the overall texture
  (54–59% positive/neutral, "cautious accumulation", no mania signals) and
  said mildly bullish — with the 0.38 mixed probability showing its work.
  Both defensible; Jev's distribution is more informative than a flat label.
- Conviction 2.53 is a better intensity read than the keyword "high": it
  separates buzz level from search-result count and puts zero mass on mania.
- Euphoria agrees (0.2 / false) — no crowded-trade flag tonight.
- surface=0.54 shows the Noul gate working as designed, but borderline —
  threshold behavior needs more samples before trusting it blindly.

## Honest caveats
- n=1 ticker, one night. Nothing generalizes yet.
- The search route returns *reporting about* WSB (ApeWisdom/AltIndex/ainvest),
  not raw WSB posts — so Jev's irony-reading advantage was barely exercised
  here. The real payoff comes with rawer inputs (Reddit OAuth upgrade).
- Cost scales with source count (2244 input tokens for 8 sources). Fine for
  a bounded shortlist; never for the full 124-symbol universe.

## Recommendation
Hybrid, as proposed: deterministic scorer always runs (free floor, works when
Jev is down); Jev Score+Noul turn runs on the shortlist; both JSONs logged to
the run dir; disagreements flagged for judge-signals to weigh. Wiring into
runbook §3c and the three scan crons (step 2c) awaits Mark's word.
