# Run 2026-09-22-close — 2026-09-22 15:12 CT (market close scan, PAPER)

Coordinator-owned run. Runbook: `docs/morning-scan-runbook.md` (coordinator paper-mode procedure).
Artifact dir: `docs/scans/2026-09-22-close/` (this scan's body-mandated location; runbook's
`runs/<run-id>/` mapping applied).

## Universe & state
- Universe scanned (scan-market --provider paper, STATIC list): BTCUSD, ETHUSD, SPY
- Positions before: paper provider empty (`[]`) — nothing held going into the close.
- Env: no credentials in scheduler env (TYPESAFE/ALPACA/KRAKEN keys absent or empty).

## Data quality
- All three: 100 bars via fetch-market-data --provider paper, normalized OK, sorted by
  timestamp_unix_ms. **SYNTHETIC bars** (paper provider generates sine-wave series with
  hardcoded start prices — engineered to trigger setups, not to reflect any market).
  Proof point: synthetic SPY close 558.52 vs real SPY ~776-780 today.

## Research sources (fresh web, coordinator's own reads)
- https://aistockwire.com/blog/stocks-record-monday-worst-breadth-since-1999-my-read-september-2026 (SentimenTrader breadth warning from Monday 09-21)
- https://www.barrons.com/livecoverage/stock-market-news-today-092226/card/s-p-500-nears-a-fresh-record-high-as-tech-stocks-gain-1BX5rm8PNkzmELvv8DeQ
- https://www.barrons.com/livecoverage/stock-market-news-today-092226/card/signals-the-ai-stock-rally-could-continue-into-year-end-9NXwHsDsAWN5WfbZR9WV
- https://www.wsj.com/livecoverage/stock-market-today-dow-sp-500-nasdaq-09-22-2026
- https://www.reuters.com/business/snapshot-nasdaq-hits-intraday-record-high-tech-stocks-regain-footing-2026-09-22/
- https://marketbeat.com/earnings/tomorrow/ ; marketbeat earnings alerts (AZO, MLKN)

## Signals generated (BollingerBands, latest candle, synthetic data — NOT actionable)
| Symbol | Side | Entry (synthetic) | SL | TP | Conf | Signal Ref |
|---|---|---|---|---|---|---|
| BTCUSD | buy | 59,467.03 | 56,825.01 | 63,048.24 | 0.64 | crypto:BTCUSD:buy:1790108026149 (Close < Lower Band 60,406.23) |
| ETHUSD | sell | 3,351.14 | 3,627.12 | 3,000.72 | 0.64 | crypto:ETHUSD:sell:1790108026310 (Close > Upper Band 3,276.70) |
| SPY | sell | 558.52 | 604.52 | 500.12 | 0.64 | equities:SPY:sell:1790108026371 (Close > Upper Band 546.12) |

All three fired off synthetic bars — proof the strategy plumbing triggers, not market signal.

## Gate verdicts
- judge-signals (direct tool call, --emit report): **EXIT 1 on all three**.
  `{"status":"error","errors":["provider error: missing TYPESAFE_API_KEY"],"data":null}`
- Gate fails closed by design. Approved: 0. Rejected: none logged by the gate itself
  (it never got to adjudicate) — the three signals above are REJECTED by default:
  **ungated signals never pass.** Recorded here as rejected-unjudged.
- No thresholds were weakened. No re-judging. No override.

## Executed
- None. No intent left judge-signals; executing on ungated signals is never acceptable.

## Outcome
**NO TRADE.** Reasons: (1) bar series are synthetic (no real market content);
(2) the deterministic judge-signals gate is unavailable without TYPESAFE_API_KEY
and fails closed — the design working as intended.

## Tradytics / TradingView legs
- Delegated to two live browser tasks (read-only, signed-in session reuse; fail-fast
  on any login wall since no unattended credential fill is possible). Results merged
  below when delivered; evidence informs future runs, never overrides the gate.

## Honest limits (restated per runbook)
1. Paper market data is synthetic — today's "signals" say nothing about real markets.
2. scan-market for paper/alpaca is a static list (BTCUSD, ETHUSD, SPY here).
3. judge-signals cannot run without TYPESAFE_API_KEY — this run legitimately ended
   at step 5 with "gate unavailable".
4. Nothing here is a live trade; paper state lives in local JSON.

## Tradytics leg (browser task, completed 2026-09-22 ~15:35 CT)
- Signed-in session was active — no fresh login needed; no credential fill performed.
- One paced pass, numbers only, no trading advice. GEX numeric fields chart-only on
  Tradytics (no text values) -> recorded missing per skill rules; 60m momentum has no
  text source -> missing.
- SPY 15:30 CT: spot 773.38; algo_flow downtrend (+2 -> -1 intraday); net call
  premium +$1.99M / net put -$0.38M; darkpool bullish, biggest 24h print 1.25M sh @
  773.35 = $966.69M (10:06:38 CT). Prophet: Bullseye 09-22 Bullish Calls 51.5% conf
  (775 strike, 9/23 exp, $718.6K prem); Stock Prophet swing 09-21 Bearish/Bearish
  79.05% conf @ 773.5. Intraday sentiment Neutral (puts ~ calls).
- QQQ 15:30 CT: spot 747.46; algo_flow uptrend (-5 -> positive); net call +$10.62M /
  net put +$1.17M; darkpool bearish (recent daily bars red), biggest 24h 1.12M sh @
  744.687 = $837.77M (11:07:33 CT); separate block-trades table: 2.28M @ 747.175 =
  $1.7B AtBid (15:01:54 CT). Prophet: Bullseye 09-22 Bullish Calls 52.6% conf
  (730 strike, 9/22 exp, $135.9K prem). Intraday sentiment Bullish (calls > puts).
- NVDA 15:30 CT: spot 228.87; algo_flow uptrend (0 -> +10-13); net call +$16.57M /
  net put +$4.16M; darkpool bullish, biggest 24h 51.87M sh @ 227.635 = $11.81B
  (2026-09-21 16:35:49 CT). Prophet: Bullseye 09-22 Bullish Calls 53.2% conf
  (220 strike, 10/16 exp, $61.15K prem). Intraday sentiment Bullish (calls > puts);
  dominant contract: 200.0 Puts @ 12/18/2026, $8.57M premiums.
- Mixed, non-directive evidence: QQQ/NVDA call-heavy and bullish intraday options
  flow, SPY neutral-to-downtrend algo flow. Informs the judge only; changes nothing
  about the gate outcome (ungated signals never pass).
