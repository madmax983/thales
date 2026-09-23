# Smoke test — no-credential paper pipeline — 2026-09-22 ~17:58 CDT

Binary: `target/debug/thales-cli` built with stable Rust 1.98.0, edition 2024 tree.
Env: `PAPER_PORTFOLIO_PATH=runs/smoke-2026-09-22/paper_portfolio.json`; no KRAKEN_/ALPACA_/TYPESAFE_ vars set.

## Commands and outcomes

| Step | Command | Exit | Result |
|---|---|---|---|
| positions | `get-positions --provider paper` | 0 | `{"status":"ok","data":[]}` — empty paper portfolio |
| universe | `scan-market --provider paper --top-n 10` | 0 | `["BTCUSD","ETHUSD","SPY"]` — STATIC list, not a live scan |
| fetch | `fetch-market-data --provider paper --symbol BTCUSD --timeframe 1h` | 0 | 100 synthetic bars, open 65000.0 → last close 59467.03. Bars are sine-wave generated, NOT market data |
| normalize | `normalize-bars --input fetch-BTCUSD.json` | 0 | same 100 bars, sorted by `timestamp_unix_ms` |
| analyze | `analyze-market --input bars-BTCUSD.json --no-report` | 0 | heuristic `MarketAnalysis` (regime/sentiment/volatility/confidence), no network |
| signals | `generate-signals --input bars-BTCUSD.json --strategy BollingerBands` | 0 | **1 signal fired on the synthetic data** (see below) — the paper generator is engineered to trigger setups |
| judge | `judge-signals --input signals-BTCUSD.json --analysis ... --bars ... --portfolio ... --emit report --log audit.md` | 1 | `{"status":"error","errors":["provider error: missing TYPESAFE_API_KEY"],"data":null}` — FAILS CLOSED as designed. No audit.md written (nothing was judged). |

## The one signal (synthetic-data artifact)

```json
{
  "intent_id": "crypto:BTCUSD:buy:1790099912895",
  "symbol": "BTCUSD", "side": "buy", "size_hint": "0.037850",
  "confidence": 0.64,
  "stop_loss": 56825.01174732183, "take_profit": 63048.244537893,
  "strategy": "BollingerBands",
  "rationale": "Strategy: BollingerBands (64%, MA: 0.80). Reason: Close 59467.03 < Lower Band 60406.23. Market Context: Trending Down (Short Term) (Medium Volatility). No similar past trades found. (Opening new position) (Risk: $100, SL Dist: 2642.02)"
}
```

Note stderr sizing debug: `Risk-based Sizing: Risk=$100.00, SL Dist=2642.0153, Calc Size=0.037850`.
`execute-intent` was deliberately NOT run: the intent never passed `judge-signals`,
and per architecture an ungated intent is never executed.

## JSON contracts observed (all commands)

Envelope: `{"status":"ok"|"error","errors":[],"warnings":[],"data":<T>}`, exit 0 on ok,
non-zero on error.

- fetch/normalize → `data`: `BarSeries{"schema_version":"v0","bars":[{symbol,market,timeframe,timestamp_unix_ms,open,high,low,close,volume}]}` (100 bars)
- analyze-market → `data`: `MarketAnalysis{regime,sentiment,volatility,confidence,recommendation,...}`
- generate-signals → `data`: `TradeIntent[]` (possibly empty); strategy intents carry stop_loss/take_profit and timestamp-suffixed intent_id `crypto:BTCUSD:buy:<ts>`
- judge-signals --emit report → `data`: `JudgeReport{approved,rejected,verdicts,thresholds,usage}`; without --emit, `data` is the surviving `TradeIntent[]` feeding execute-intent

## Honest bottom line

The no-credential paper run exercises the plumbing end-to-end but proves nothing
about real markets: paper bars are synthetic and biased to fire signals, and the
gate correctly refused to adjudicate without `TYPESAFE_API_KEY`. A real morning
scan needs Kraken/Alpaca credentials for data and `TYPESAFE_API_KEY` for the gate —
or it honestly reports "gate unavailable" and does nothing.
