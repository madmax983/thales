# SPEC-005: Volatility Forecasting (EWMA now, GARCH next)

**Status**: v0 built 2026-09-23 (EWMA, `forecast-volatility`, `--history`, `adjusted_close`); v1 GARCH(1,1) still open
**Owner**: Mark
**Created**: 2026-09-23

## 1. User Story
> "As the Thales coordinator, I want a forward-looking volatility forecast per
> candidate symbol, so that regime labels, position sizing, and the judge's
> regime-fit check are driven by an estimated number instead of a heuristic
> vibe."

## 2. The "So What?"
- **Problem**: `analyze-market` labels volatility heuristically. Bollinger
Bands use *backward-looking* rolling σ, so bands widen *after* the vol
event. The Signal Generator's "size on volatility" rule currently has only
ATR-flavored inputs.
- **Solution**: A `VolatilityEstimator` trait with two estimators —
**EWMA** (RiskMetrics, ships in v0) and **GARCH(1,1)** (MLE, ships in v1)
— exposed as `thales-cli forecast-volatility`, fed by deep adjusted-close
history.
- **Value**: Bands that widen *before* vol events, vol-targeted sizing, and a
judge that sees a real forecasted σ when it checks regime fit. This is the
missing quantitative leg under the scan pipeline.

## 3. Gap Analysis
- **Current state**: heuristic volatility labels; Bollinger σ is a trailing
20-bar sample standard deviation; no forward vol estimate anywhere.
- **Desired state**: every shortlist candidate gets a 1-day-ahead σ forecast
(daily + annualized) from fitted history, folded into research evidence the
judge already reads. No gate signature changes.

## 4. Historical Data — Research (2026-09-23)

GARCH(1,1) needs **≥250 daily bars** of **split/dividend-adjusted** closes to
fit without embarrassing itself (rule of thumb; fewer → fail closed, see §8).
EWMA needs only ~60. Sources evaluated against the 124-symbol universe
(equities, ETFs, ETNs, indices, futures, crypto):

| Source | Daily depth | Adjusted? | Universe coverage | Auth / cost | Verdict |
|---|---|---|---|---|---|
| **Yahoo chart API** (`range=max`) | Decades — 11,497 daily rows verified for AAPL, back to IPO for most tickers | Yes — `adjclose` + div/split events | All classes incl. `^`-indices, `=F` futures, `-USD` crypto | No key; unofficial, no SLA, 429s possible | **PRIMARY** — already integrated, no new credential |
| **Tiingo** | 30+ years | Yes — split+div adjusted | US equities/ETFs (80k assets); indices/futures TBD | Free key: 500 sym/mo, 50 req/hr, 1000/day | **Fallback / cross-validation** — one API key via Secure Vault when Yahoo quality wobbles |
| **Binance public klines** | Deep, exchange-dependent | N/A (crypto) | Crypto only, no key needed | None | Crypto deep-history alternative; keep Yahoo unless needed |
| Stooq | ~20y daily | Yes | Good | **EXCLUDED** — JS proof-of-work challenge since ~Jun 2026 blocks all non-browser clients |
| Massive (ex-Polygon) free | 2 years | Yes | US equities | None | Too thin for GARCH; excluded as primary |
| Alpha Vantage free | 20+ years | Yes | Good | 25 req/day | Quota too thin for scans; excluded |
| Alpaca | Since 2016 | Yes | US equities/ETFs | Vault-blocked for us (see 2026-09-23 log) | Excluded until the two-connector issue is resolved |

**Recommendation**: extend the existing `yahoo` provider — no new credential,
whole-universe coverage, one gentle pass per scan. Add Tiingo only if Yahoo
data-quality issues appear in audits.

### 4.1 Provider change required
`crates/providers/yahoo` currently hardcodes `1d → range=6mo` (~126 bars:
fine for signals, **insufficient for GARCH**). Add an optional `--history`
flag to `fetch-market-data`:

```bash
thales-cli fetch-market-data --provider yahoo --symbol SPY --timeframe 1d --history 2y
# → interval=1d&range=2y (~500 bars; satisfies the GARCH minimum)
```

`--history` accepts `1y 2y 5y 10y max` (default: the current per-timeframe
mapping, so existing scan behavior is unchanged). Also parse the `adjclose`
array into an optional `adjusted_close` field on `Bar` (contracts change,
additive only) — log returns for fitting **must** come from adjusted closes,
falling back to `close` with a warning.

## 5. Specification

### 5.1 New crate: `crates/volatility` (edition 2024)
Pure Rust, no new native dependencies.

```rust
pub trait VolatilityEstimator {
fn name(&self) -> &'static str;
fn min_bars(&self) -> usize;
fn fit(&self, log_returns: &[f64]) -> Result<VolatilityFit, FitError>;
}

pub struct VolatilityFit {
pub estimator: String, // "ewma" | "garch11"
pub nobs: usize,
pub params: BTreeMap<String, f64>, // lambda | omega, alpha, beta
pub forecast_variance_1d: f64, // σ²_{t+1}
pub forecast_vol_daily: f64, // σ_{t+1}
pub forecast_vol_annual: f64, // σ_{t+1} * sqrt(periods_per_year)
pub persistence: f64, // λ | α+β
pub half_life_bars: f64,
pub log_likelihood: Option<f64>,
pub warnings: Vec<String>, // e.g. "boundary solution", "thin history"
}

pub enum FitError {
InsufficientData { have: usize, need: usize},
NonFiniteInput,
NoConvergence,
}
```

**Estimators**:
- **v0 — `Ewma { lambda: 0.94}`**: σ²_t = λσ²_{t-1} + (1−λ)r²_{t-1},
seeded with sample variance. Closed form, deterministic, ~60 bars minimum.
- **v1 — `Garch11`**: σ²_t = ω + αr²_{t-1} + βσ²_{t-1}, Gaussian MLE with
constraints ω>0, α≥0, β≥0, α+β<1 enforced by parameter transform +
Nelder-Mead (document the optimizer choice in code). 250 bars minimum.
Boundary/non-convergent fits return `warnings`, never silent garbage —
the coordinator falls back to EWMA and records why.
- **v2 (stretch) — `GjrGarch`**: adds the leverage term for the equity
asymmetric-volatility effect. Not in v0/v1 scope.

### 5.2 CLI command: `forecast-volatility`

```bash
thales-cli forecast-volatility \
--input runs/<run-id>/bars-SPY.json \
--estimator ewma \
[--lambda 0.94] \
[--annualization 252] \
[--min-bars 60]
```

- Reads a normalized `BarSeries`; builds log returns from `adjusted_close`
(warns and falls back to `close` if absent).
- `--annualization`: 252 for equities/ETFs/indices/futures, **365** for
crypto (24/7) — the coordinator sets it from the universe manifest's
asset class.
- Output is the standard JSON envelope; `data` is the `VolatilityFit`.
- Exit non-zero on `FitError` (fail closed — never invent a forecast).

### 5.3 Runbook integration (no gate changes)
New step **§2b — Volatility forecast**, after `normalize-bars`:

```bash
# one deep fetch per shortlist symbol; the tail serves signals too (single pass)
$BIN fetch-market-data --provider yahoo --symbol SPY --timeframe 1d --history 2y > runs/<run-id>/hist-SPY.json
$BIN normalize-bars --input runs/<run-id>/hist-SPY.json > runs/<run-id>/bars-SPY.json
$BIN forecast-volatility --input runs/<run-id>/bars-SPY.json --estimator ewma \
--annualization 252 > runs/<run-id>/vol-SPY.json
```

- v0 runs EWMA on the same bars (no extra fetch).
- v1 switches `--estimator garch11` once the estimator ships; the 2y
history already satisfies its minimum.
- The forecast's headline numbers (forecast σ daily/annual, persistence,
half-life, warnings) are folded into the `--research` text for
`analyze-market`, exactly like Tradytics/WSB evidence — **the judge sees
them without any gate signature change**.
- Sizing: the Signal Generator's "size on volatility" rule may consume
`forecast_vol_daily` for vol targeting in v2. Not in v0/v1 scope.

### 5.4 Metrics & success criteria
- EWMA forecast matches a hand-computed reference on a fixed fixture to
1e-12.
- GARCH(1,1) recovers known (ω, α, β) from seeded synthetic GARCH data
within documented tolerance bands.
- Same input → byte-identical output (determinism; no wall-clock, no RNG
without seed in the fit path).
- `cargo test -p volatility` green on stable; `cargo fmt --check` clean;
edition 2024.

### 5.5 Tests (fixtures only — no network, per repo rule)
- EWMA closed-form check on a 5-return fixture.
- GARCH parameter recovery on synthetic data (seeded).
- Constraint enforcement: α+β<1, ω>0 on adversarial inputs.
- `InsufficientData` below minimums (59 bars EWMA, 249 bars GARCH) —
fails closed, never panics.
- Non-finite input (NaN/Inf) → `NonFiniteInput`.
- Adjusted-close preference: fixture with a split event proves returns
come from `adjusted_close`.

## 6. Rollout
- **v0**: `crates/volatility` + EWMA, `forecast-volatility` CLI,
`--history` + `adjclose` in the yahoo provider, runbook §2b, tests.
- **v1**: GARCH(1,1) estimator, coordinator flips shortlist to
`--estimator garch11` with EWMA fallback on warnings.
- **v2**: GJR-GARCH asymmetry; vol-targeted sizing in the Signal Generator;
Tiingo cross-validation if Yahoo quality issues surface in audits.

## 7. Honest Limits
- GARCH on <250 bars is numerology — the minimums are fail-closed, not
advisory.
- Weekly bars (2y ≈ 104) can never feed GARCH(1,1); EWMA-only there.
- MLE can land on boundary solutions (α+β→1, "IGARCH-like"); the
`warnings` field exists for exactly this — the coordinator must surface
it, not smooth it over.
- Yahoo is unofficial/no-SLA: one gentle pass per scan; deep-history
fetches are per-symbol and bounded by the shortlist (never the full 124).
- Futures continuous contracts (`ES=F`) have roll artifacts Yahoo does not
fully neutralize — treat index-future vol forecasts as approximate.
- Crypto annualization uses 365; mixing 252/365 across a portfolio is the
coordinator's bookkeeping job, not the estimator's.
- A vol forecast is not a trade signal. It informs regime, sizing, and the
judge's evidence — it never clears the gate by itself.
