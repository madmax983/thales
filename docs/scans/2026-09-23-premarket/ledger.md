# Run 2026-09-23-premarket — 2026-09-23 07:12 CT (premarket scan, PAPER)

Coordinator-owned run. Runbook: `docs/morning-scan-runbook.md` (coordinator paper-mode procedure).
Artifact dir: `docs/scans/2026-09-23-premarket/` (body-mandated location; runbook's `runs/<run-id>/` mapping applied).

## Universe & state
- Universe scanned (scan-market --provider paper, STATIC manifest list, --top-n 10): JETS, DIA, DJX, EEM, IJR, IWM, MDY, NDX, QQQ, RSP
- Positions before: paper provider empty (`[]`) — nothing held going into the session.

## Data quality
- All ten: 100 bars via fetch-market-data --provider paper, normalized OK, sorted by timestamp_unix_ms, latest 2026-09-23 12:13 UTC (07:13 CT, fresh).
- **SYNTHETIC bars** (paper provider generates sine-wave series with hardcoded start prices — engineered to trigger setups, not to reflect any market). Proof: all ten symbols share the identical series shape and last close 167.56 (vs real QQQ 745.08, DIA 518.00, IWM 287.21).

## Research sources (fresh web, coordinator's own reads)
- https://wncy.com/2026/09/23/wall-st-futures-steady-with-focus-on-mideast-talks-us-china-summit/ (Reuters: futures steady; Dow E-minis +31pts +0.06%, ES E-minis +6.75pts +0.09%, NQ E-minis +1.75pts +0.01% at 5:25am ET; Mideast talks, US-China summit Thursday, Xi state visit Wednesday; Fed raised rates last week, restrictive bias; AI optimism, Meta Muse; Financials biggest daily drop since March Tuesday)
- https://www.barrons.com/articles/s-p-500-futures-flat-in-premarket-trading-ionq-quantinuum-lead-585ff1ab (premarket movers: IONQ +11.7%, QNT +6.5%, PACS/LEN.B/WMS +5%+, AROC -7.9%, IMVT -7.4%; 10Y 4.979%; BTC $85,565 -0.77%; S&P flat prev session, Dow -0.36%)
- https://www.wsj.com/livecoverage/stock-market-today-dow-sp-500-nasdaq-09-23-2026/card/trump-xi-meeting-costco-earnings-what-to-watch-the-rest-of-the-week-slC0magDDnkTnqfM3UUT (Wed: Mfg+Services PMI flash, MBA mortgage apps, EIA petroleum report, Fed Gov Barr; earnings BMO GIS/CBRL/PAYX/CTAS, AMC SFIX; Thu: Trump-Xi, COST/DRI/BB earnings)
- https://www.tipranks.com/news/options-volatility-and-implied-earnings-moves-this-week-september-22-september-24-2026 (implied earnings moves: CTAS +/-5.32%, GIS +/-8.62%, PAYX +/-7.63%, SFIX +/-20.42%)
- WSB mania meter: ApeWisdom (http://apewisdom.io/stocks/{QQQ,DIA,IWM,IONQ}/), https://altindex.com/wallstreetbets, https://www.memebergterminal.com/stocks/subreddits/wallstreetbets

## Signals generated (BollingerBands, latest candle, synthetic data — NOT actionable)
All ten fired IDENTICAL BollingerBands sell signals off the identical synthetic series:
side sell | entry 167.56 | SL 181.36 | TP 150.04 | conf 0.64 | rationale "Close > Upper Band 163.83"
(symbols JETS/DIA/DJX/EEM/IJR/IWM/MDY/NDX/QQQ/RSP; intent_ids equities:<SYM>:sell:<ts>)
This proves the strategy plumbing triggers — it says nothing about real markets.

## Gate verdicts — judge-signals via ./scripts/judge-with-jev.sh (direct tool call, --emit report)
Credential wrapper worked: the gate reached the Jev API and returned real adjudicated verdicts.
**Approved: 0. Rejected: 10/10.** Rows in `audit.md` (written by the gate itself, --log).
| Symbol | Verdict | Instrument quality | Confidence | Skip p | Regime fit | Conviction |
|---|---|---|---|---|---|---|
| QQQ | skip | 0.96 | 0.69 | 0.79 | 0.25 | Weak (1.56) |
| JETS | skip — instrument-quality veto 0.47 < 0.50 floor | 0.47 | — | 0.74 | 0.29 | Weak |
| DIA | skip | 0.93 | — | 0.78 | 0.25 | Weak |
| DJX | skip | 0.73 | — | 0.74 | 0.28 | Weak |
| EEM | skip | 0.86 | — | 0.78 | 0.27 | Weak |
| IJR | skip | 0.88 | — | 0.79 | 0.26 | Weak |
| IWM | skip — confidence 0.55 < 0.60 floor | 0.91 | 0.55 | 0.70 | 0.27 | Weak |
| MDY | skip | 0.73 | — | 0.77 | 0.27 | Weak |
| NDX | skip | 0.93 | — | 0.74 | 0.28 | Weak |
| RSP | skip — confidence 0.57 < 0.60 floor | 0.88 | 0.57 | 0.72 | 0.28 | Weak |
No thresholds were weakened. No re-judging. No override. The gate correctly refused
synthetic-data signals (regime-fit 0.25–0.29 across the board: the analyses read
"Trending Up (Short Term)" while the strategy signaled short — the veto logic worked).

## Executed
- None. Nothing survived the gate; executing ungated signals is never acceptable.

## Outcome
**NO TRADE.** Reasons: (1) all bar series are synthetic (no real market content);
(2) the deterministic judge-signals gate, running credentialed for the first time
in a premarket scan, rejected every signal — two on floor violations
(JETS instrument-quality veto, IWM/RSP confidence floor) and all ten on the model's
own skip verdict. The design worked end to end.

## Tradytics leg (fetch-with-cookie.sh, paced, exit 0 on all 6 pulls)
Data is dated 2026-09-22 (yesterday's session) — recorded as such; premarket today,
so yesterday's close is the freshest options positioning available.
- QQQ (Tradytics spot 747.46, 9/22): net call prem +$10.30M / net put +$4.27M; net GEX +$0.40B; call walls 745/747/748/750 ($858M–$3.7B +gamma); negative gamma below at 660/700/730/742. Biggest prints: 780 puts 12/18/2026 $2.66M sell-side; 745 calls 10/16/2026 $5.14M.
- DIA (spot 518.00, 9/22): net call prem +$0.58M / net put +$1.58M; net GEX +$0.02B (thin); -gamma at 510/515/520/521, +gamma at 525/530/540/550.
- IWM (spot 287.21, 9/22): net call prem +$3.63M / net put +$8.18M (put-heavy hedging); net GEX +$0.20B; +gamma clustered near spot 287–293; large -gamma below at 272/275/280/285. Biggest prints: 272/274 puts 10/16/2026 ($0.91M/$1.17M).
Mixed, non-directive evidence. Informs; judge decides (changes nothing — all gated out).

## TradingView leg (signed-in browser task, completed 07:23 CT; exchange corrections: QQQ=NASDAQ, DIA=AMEX, IWM=AMEX)
Chart template had no indicators → 15m RSI/MACD/EMA/ATR marked missing, not inferred.
- ES1! 07:17 CT: 7,819.75, -0.15% (-12.00); day range 7,799.25–7,857.50; OI 1.89M; front ESZ2026. Daily RSI 61.06; MACD Level(12,26) 30.31 Buy; EMA20 7,714.87; EMA50 7,655.11. Resistance 7,844.08 (Daily Classic R1), support 7,715.42 (pivot). Rating: sidebar Buy / main Neutral / MAs Strong buy (1d).
- NQ1! 07:18 CT: 30,921.75, -0.34% (-106.75); day range 30,764.00–31,052.25; OI 286.60K; front NQZ2026. Daily RSI 68.49 (elevated); MACD 305.60; EMA20 29,826.27; EMA50 29,525.17. Resistance 31,419.33 (R2), support 30,466.17 (R1). Rating: sidebar Buy / main Neutral; oscillators Sell(14) / MAs Strong buy(13).
- QQQ 07:20 CT: premarket 747.46 +0.81% (+5.99); regular close 745.08 -0.32%. Rating Strong buy.
- DIA 07:19 CT: premarket 516.60 -0.27%; regular close 518.00 -0.34%. Rating Sell.
- IWM 07:21 CT: premarket 285.51 -0.59%; regular close 287.21 +0.57%. Rating Sell.
Futures slid from flat to mildly red after 5:25am ET (ES -0.15%, NQ -0.34% by 07:18 CT). Futures window informs only — no futures candidates were in the Thales universe top-10, and no futures swing setup was generated.

## WSB leg (skill §3c: browser.search ×4 tickers, deterministic scorer + Jev turn on all 4)
- QQQ: keyword lean mixed (net +1, low vol); Jev lean bullish 0.77 (mixed 0.09, bearish 0.12), conviction 1.63, euphoria 0.23, surface 0.34. **Lean disagreement recorded** (keyword mixed vs Jev bullish). Mild retail bullishness, attention cooling (mentions -36%).
- DIA: quiet on both scorers (Jev quiet 1.00, conviction 0.00).
- IWM: keyword mixed; Jev lean bearish 0.80 (mixed 0.13, quiet 0.07), conviction 1.96, euphoria 0.13. Slight retail bearishness on small caps.
- IONQ (premarket mover, not a Thales candidate): keyword mixed (net +1, medium vol); Jev lean bullish 1.00, conviction 3.74, **euphoria 0.89**, surface 0.76. ApeWisdom: 25 WSB mentions +2,700%, 90% positive; AltIndex: mentions +1,750%, "currently trending on WallStreetBets"; Barron's: +11.7% premarket. Read CONTRARIAN: crowded-trade flag, not confirmation. Watchlist context only — no Thales pipeline data for IONQ.
WSB evidence is advisory; changes nothing about gate outcome.

## Honest limits (restated per runbook)
1. Paper market data is synthetic — today's "signals" say nothing about real markets.
2. scan-market for paper is a static list (10 symbols this run).
3. judge-signals ran credentialed via the wrapper — the gate worked and rejected everything. That is the design working.
4. Nothing here is a live trade; paper state lives in local JSON.

## Watchlist for the session (setups to monitor, not entries to take)
- ES/NQ futures: mildly red premarket, above all daily MAs (ES EMA20 7,714.87 / EMA50 7,655.11; NQ EMA20 29,826.27 / EMA50 29,525.17); RSI elevated on NQ (68.49). Watch 7,844 / 31,419 resistance and 7,715 / 30,466 support into the open; Thursday's Trump-Xi summit is the headline risk.
- QQQ: premarket +0.81% bouncing off record-high territory; heavy +gamma 745–750 could pin or accelerate; WSB retail attention cooling.
- IWM: put-heavy hedging, -gamma walls below spot 272–285; premarket -0.59%; WSB retail lean slightly bearish — small-cap weakness watch.
- IONQ: +11.7% premarket on a WSB pile-on (conviction 3.74, euphoria 0.89) — crowded-trade caution; NOT a Thales candidate, no signal was generated.
- Earnings reactions to watch at the open: GIS, CBRL, PAYX, CTAS (BMO); data at the bell: PMI flash, mortgage apps.
