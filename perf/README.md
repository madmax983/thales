# Performance harness

`bolt_benchmark.sh` drives the CLI's `benchmark` command
(`crates/cli/src/benchmark.rs`), which runs every registered strategy's
`generate_signals` + backtest simulation over a bar series. This is the
heaviest realistic path in the codebase: a single invocation walks ~80
strategies, each computing several technical indicators over the full bar
series.

## Reproduce

```sh
cargo build --release -p thales-cli --features nova
perf/bolt_benchmark.sh              # wall-clock timing (context only)
perf/bolt_benchmark.sh --callgrind  # instruction counts via valgrind --tool=callgrind
```

The input (`perf/fixtures/synthetic_bars_5000.json`) is a committed,
fixed 5,000-bar synthetic hourly series (via
`thales_cli::synthetic_data::generate_synthetic_data`) so instruction counts
are comparable across commits without regenerating random data.

Wall-clock numbers are **not** admissible evidence on this hardware (shared
vCPU); only the `--callgrind` instruction counts are used to gate changes.

## Baseline (before any optimization)

Recorded with `valgrind-3.22.0`, `perf/bolt_benchmark.sh --callgrind`,
release build of `thales-cli` (`--features nova`), on the fixture above.

```
I refs: 8,842,303,257
```

Top self-cost functions (`callgrind_annotate --threshold=80`):

| Ir | % | Function |
|---:|---:|---|
| 3,883,505,091 | 43.92% | `rust_decimal::decimal::base2_to_decimal` |
| 675,335,438 | 7.64% | `rust_decimal::ops::common::Buf24::rescale` |
| 628,456,087 | 7.11% | `rust_decimal::ops::div::div_impl` |
| 385,619,355 | 4.36% | `rust_decimal::ops::add::add_sub_internal` |
| 320,011,079 | 3.62% | `rust_decimal::ops::mul::mul_impl` |
| 291,273,489 | 3.29% | `polars_core::chunked_array::ChunkedArray<T>::get` |
| 257,574,602 | 2.91% | `rust_decimal::ops::add::unaligned_add` |
| 229,682,677 | 2.60% | `Decimal::to_f64` |
| 198,798,983 | 2.25% | `rust_decimal::ops::add::aligned_add` |
| 139,258,782 | 1.57% | `backtest::run_backtest_with_strategy::{{closure}}` |

`rust_decimal` internals alone account for **~74%** of all instructions
executed by this workload. Every technical indicator converts each `f64`
input to `rust_decimal::Decimal` per bar, accumulates with software decimal
arithmetic, then converts back to `f64` on output (see e.g.
`crates/strategies/src/indicators/sma.rs`, `.../atr.rs`) — precision that is
immediately discarded on the `f64` round-trip.

`indicators::atr::calculate` is the single highest-leverage target: it is
called by 79 of ~85 strategies (nearly every strategy uses ATR for
stop-loss sizing) and performs 3 `Decimal::from_f64_retain` conversions per
bar across the full series.

## After: ATR indicator rewritten in native f64

`crates/strategies/src/indicators/atr.rs` rewritten to compute Wilder's
smoothing entirely in `f64` instead of `rust_decimal::Decimal` (algorithm
and NaN handling unchanged; see commit history for the diff). Same
harness, same fixture, same machine, same session:

```
I refs: 6,790,514,448   (baseline: 8,842,303,257)
```

| | Ir | % of baseline |
|---|---:|---:|
| Baseline | 8,842,303,257 | 100.00% |
| After ATR fix | 6,790,514,448 | 76.80% |
| **Delta** | **-2,051,788,809** | **-23.20%** |

Clears the impact floor (≥5% instruction reduction) by a wide margin.
`base2_to_decimal` self-cost drops from 43.92% to 38.14% of the (now
smaller) total, and the other `rust_decimal` ops functions shrink
proportionally — consistent with removing ATR's 3-conversions-per-bar
across ~93% of strategies while leaving every other indicator's Decimal
usage untouched. Those remaining indicators (`sma`, `ema`, `rsi`, etc.)
are separate, smaller-diff follow-ups; see the PR for scope notes.

## Baseline for this run (before the strategy-layer Decimal fix)

Recorded with `valgrind-3.22.0`, `perf/bolt_benchmark.sh --callgrind`,
release build of `thales-cli` (`--features nova`), same fixture, same
machine, same session, at the commit that includes the ATR fix above:

```
I refs: 6,793,524,674
```

`rust_decimal` internals are still the dominant cost, now **~69%** of
total instructions:

| Ir | % | Function |
|---:|---:|---|
| 2,589,703,962 | 38.12% | `rust_decimal::decimal::base2_to_decimal` |
| 624,008,520 | 9.19% | `rust_decimal::ops::common::Buf24::rescale` |
| 547,913,757 | 8.07% | `rust_decimal::ops::div::div_impl` |
| 293,966,920 | 4.33% | `rust_decimal::ops::add::add_sub_internal` |
| 283,077,711 | 4.17% | `rust_decimal::ops::mul::mul_impl` |
| 215,308,711 | 3.17% | `rust_decimal::ops::add::unaligned_add` |
| 165,616,912 | 2.44% | `Decimal::to_f64` |
| 135,960,091 | 2.00% | `rust_decimal::ops::add::aligned_add` |
| 74,532,512 | 1.10% | `Decimal::from_f64_retain` |

`indicators::*::calculate` self-costs are now all small (each &lt;0.5% —
see the per-function breakdown in this session's PR), so the next
highest-leverage target is **not** in the shared indicator layer, it's in
the *strategy* layer: a `callgrind_annotate --tree=caller` walk of
`Decimal::from_f64_retain` (the single largest caller edge into
`base2_to_decimal`, 2,436,610,687 Ir / **35.87%** of total, 2,191,040
calls) shows that edge is fed almost uniformly by ~85 distinct
`<Strategy as Strategy>::generate_signals::{{closure}}` symbols, each
contributing ~11.8M Ir via ~9,900 calls. Reading the source confirms the
mechanism: unlike the indicator layer (fixed above), the strategy layer
never centralized this pattern in a shared helper — each strategy's
`generate_signals` independently duplicates the same round-trip inline:
convert the current-bar `close`/ATR `f64` values (already `Option<f64>`
from the polars column) to `rust_decimal::Decimal` via
`.and_then(Decimal::from_f64_retain)`, do the stop-loss / take-profit
arithmetic (`+`/`-`/`*` against a `Decimal`-ified config multiplier) in
`Decimal`, then immediately call `.to_f64().unwrap_or(0.0)` because
`strategy::Signal::stop_loss`/`take_profit` are plain `f64` fields. The
`Decimal` precision is discarded on every call — same defect class as the
ATR fix, just duplicated per-strategy instead of centralized in one
function. Confirmed by source inspection of 49 strategy files matching
`grep -rl 'Decimal::from_f64_retain' crates/strategies/src | grep -v
indicators/` (e.g. `crates/strategies/src/williams_r.rs:114-197`,
`crates/strategies/src/macd.rs:124-181`,
`crates/strategies/src/adl_momentum.rs:90-119`): every one of the
sampled files follows the identical shape — no algorithmic use of
`Decimal`'s arbitrary precision (no rounding-mode-sensitive output, no
persisted-decimal accumulator), just a discarded round-trip.

One file, `crates/strategies/src/bollinger_bands.rs`, is excluded from
this fix: it uses a `Decimal` accumulator for incremental
sum/sum-of-squares/variance tracking across the whole series (not a
per-bar round-trip), with an explicit source comment noting the author
chose `Decimal` because "variance might be slightly negative due to
precision if using f64, but with Decimal should be fine" — a real
precision-sensitive numerical-stability concern (a negative variance
would feed `.sqrt()` and produce `NaN`/wrong signals). That needs
case-by-case numerical analysis (e.g. Welford's algorithm), not a
mechanical swap, so it's left as a follow-up; see the PR for details.

## After: strategy-layer stop-loss/take-profit round-trip removed

47 of the 49 candidate files converted (mechanical swap: drop the
`Decimal::from_f64_retain` / `.to_f64()` round-trip around the
`close`/ATR-derived stop-loss and take-profit arithmetic, operate on the
`f64` values directly). Two files were intentionally left untouched
because their `Decimal` usage doesn't match the discarded-round-trip
shape:

- `crates/strategies/src/pvi_trend.rs` and
  `crates/strategies/src/weighted_close_trend.rs` parse `Decimal` values
  from a *string*-typed column (`Decimal::from_str`, not
  `from_f64_retain`) and, in `weighted_close_trend.rs`, carry a running
  sum accumulator in `Decimal` across bars — the same "real precision use,
  not a discarded round-trip" pattern as `bollinger_bands.rs` above.
  Follow-up, not touched here.

A handful of the 47 converted files (`ema_crossover.rs`,
`kama_crossover.rs`, `macd.rs`, `triple_sma_crossover.rs`,
`hma_macd_trend.rs`) only partially converted: the *comparison* values
(e.g. short/long EMA, MACD/signal, HMA) are displayed via
`Decimal::round_dp(2)` in the signal's `reason` string, so those specific
bindings were left as `Decimal` while the independent price/ATR/output
stop-loss arithmetic was still converted. This is why the win is smaller
than the 35.87% upper bound estimated from the `from_f64_retain` caller
tree in the previous section: a large share of that edge's calls were
converting *indicator values* for display/comparison, not the
price/ATR/output values this fix targets.

## Baseline for this run (before the indicator-layer Decimal fix)

Recorded with `valgrind-3.22.0`, `perf/bolt_benchmark.sh --callgrind`,
release build of `thales-cli` (`--features nova`), same fixture, same
machine, same session, at the commit that includes the strategy-layer fix
above:

```
I refs: 6,155,105,907
```

`rust_decimal` internals are still ~73% of total instructions — now the
dominant cost is entirely in the shared indicator layer (`sma`, `ema`,
`rsi`, etc.), exactly as flagged as a follow-up in the previous section:

| Ir | % | Function |
|---:|---:|---|
| 2,024,588,995 | 32.89% | `rust_decimal::decimal::base2_to_decimal` |
| 614,022,121 | 9.98% | `rust_decimal::ops::common::Buf24::rescale` |
| 547,913,757 | 8.90% | `rust_decimal::ops::div::div_impl` |
| 290,865,214 | 4.73% | `rust_decimal::ops::add::add_sub_internal` |
| 276,983,125 | 4.50% | `rust_decimal::ops::mul::mul_impl` |
| 240,321,886 | 3.90% | `polars_core::chunked_array::ChunkedArray<T>::get` |
| 211,099,798 | 3.43% | `rust_decimal::ops::add::unaligned_add` |
| 159,577,844 | 2.59% | `Decimal::to_f64` |
| 134,483,308 | 2.18% | `rust_decimal::ops::add::aligned_add` |
| 65,382,480 | 1.06% | `rust_decimal::ops::cmp::cmp_impl` |
| 57,695,542 | 0.94% | `Decimal::from_f64_retain` |
| 53,982,159 | 0.88% | `rust_decimal::ops::common::Buf12::find_scale` |
| 47,417,364 | 0.77% | `rust_decimal::ops::cmp::cmp_internal` |

`rust_decimal` internal functions alone sum to **~72.9%** of total
instructions. Per-indicator self-cost is small and diffuse (no single
`indicators::*::calculate` function exceeds 0.5% self-cost — see
`crates/strategies/src/indicators/sma.rs`, `.../ema.rs`, `.../rsi.rs` for
representative examples), because the cost is spread across ~44 indicator
files that each independently perform the same discarded round-trip: an
`f64` input is converted to `rust_decimal::Decimal` via
`Decimal::from_f64_retain`, accumulated/smoothed/multiplied in `Decimal`
across the bar series, then converted back with `.to_f64().unwrap_or(0.0)`
because every indicator's public contract is a `Series` of `f64`. Same
defect class as the ATR and strategy-layer fixes above, just duplicated
across the indicator layer instead of centralized.

Of the ~57 indicator files that import `rust_decimal`, 13 are excluded
from this fix because they have a genuine precision-sensitive reason to
use `Decimal` (confirmed by source inspection, not assumption):

- `bollinger_bands.rs`, `stddev.rs`, `zscore.rs`, `ulcer_index.rs` compute
  a sum-of-squares/variance in `Decimal` before `.sqrt()`. This is the
  same numerical-stability concern documented for `bollinger_bands.rs` in
  the previous section (`f64` catastrophic cancellation can make the
  variance spuriously negative, and `.sqrt()` of a negative number is
  `NaN`) — not a discarded round-trip.
- `true_range.rs`, `typical_price.rs`, `average_price.rs`,
  `median_price.rs`, `weighted_close.rs`, `aroon_custom.rs`, `pvi.rs`,
  `vroc.rs`, `vwmo.rs` cast their input columns to `String` and parse with
  `Decimal::from_str` specifically *to avoid* `f64` precision loss at the
  boundary (see the doc comment in `true_range.rs`: "Cast inputs to String
  to avoid any f64 float precision loss at the boundary"), and several of
  them (`true_range.rs`, `typical_price.rs`) return `Series` of `String`,
  not `f64` — the `Decimal` precision is preserved all the way to the
  output, so there is no round-trip to remove.

The remaining files (`adl`, `adx`, `alma`, `bop`, `chaikin_oscillator`,
`chandelier_exit`, `choppiness_index`, `cmf`, `cmo`, `disparity_index`,
`donchian_channels`, `dpo`, `eom`, `fisher_transform`, `force_index`,
`gator`, `ichimoku`, `kama`, `kdj`, `keltner_channels`, `kst`,
`linear_regression`, `mfi`, `momentum`, `nvi`, `obv`, `ppo`, `qstick`,
`roc`, `rsi`, `rvi`, `sma`, `smma`, `stc`, `stoch_rsi`, `stochastic`,
`supertrend`, `ultimate_oscillator`, `vhf`, `vpt`, `vwap`, `vwma`, `wma`,
`zlema` — 44 files) match the discarded-round-trip shape: `f64` in,
`Decimal` arithmetic with no persisted precision benefit, `f64` out via
`.to_f64().unwrap_or(0.0)`. None use `round_dp` for display (that pattern
only existed in the strategy layer), so no partial-conversion carve-out
is needed this time.

One file initially in this list, `ema.rs`, was converted and then
**reverted** after it broke an existing test — see the negative-result
note below. `smma.rs` needed a numerically-stable reformulation (not a
plain type swap) to match the original's output bit-for-bit; see its note
below too.

## After: 44 indicator files converted to native f64

Same harness, same fixture, same machine, same session:

```
I refs: 2,624,688,881   (this run's baseline: 6,155,105,907)
```

| | Ir | % of this run's baseline |
|---|---:|---:|
| Baseline (post-strategy-layer-fix) | 6,155,105,907 | 100.00% |
| After indicator-layer fix | 2,624,688,881 | 42.64% |
| **Delta** | **-3,530,417,026** | **-57.36%** |

Clears the impact floor (≥5% instruction reduction) by more than 11x.
`rust_decimal` internals are still present (`base2_to_decimal` 20.15%,
`Buf24::rescale` 8.00%, `div_impl` 6.33%, `mul_impl` 3.57%,
`add_sub_internal` 2.52%, `unaligned_add` 2.29%, `Decimal::to_f64`
2.08% — ~45% of the new, smaller total) but now entirely attributable to
the 13 files excluded above for genuine precision reasons, plus
`ema.rs` (kept as `Decimal` — see below). `polars_core::ChunkedArray::get`
(9.16%) and `thales_cli::backtest::run_backtest_with_strategy::{{closure}}`
(5.30%) are now among the largest single entries, i.e. cost has shifted
from "indicator arithmetic" to "reading columns and running the backtest
loop itself" — the next follow-up, if any, is in that direction rather
than in `rust_decimal`.

### Negative result: `ema.rs` reverted

`ema.rs` matched the discarded-round-trip pattern exactly like every
other file in this batch (input `f64` → `Decimal::from_f64_retain` →
Wilder-style exponential smoothing in `Decimal` → `.to_f64()` on output,
same shape as the already-fixed `atr.rs`), and its own unit tests passed
unchanged after conversion. But the full workspace suite
(`cargo test --workspace --all-features`) caught one failure:
`double_ema_crossover::tests::test_entry_and_exit_signals`, which feeds
synthetic linear-ramp price data (up 50 bars, down 50 bars) through
`dema::calculate` (`DEMA = 2*EMA - EMA(EMA)`, i.e. `ema::calculate`
applied twice in series) and asserts on exact crossover-driven
entry/exit signals.

Root cause, confirmed by instrumenting the strategy's signal loop and
diffing per-bar EMA output between the `Decimal` and `f64` versions: the
original `Decimal` implementation keeps `prev_ema` as a `Decimal` across
the whole series (not rounded to `f64` until the final push), so it
carries ~28-29 significant decimal digits of internal precision through
every iteration, compounding differently than an `f64` accumulator
(~15-17 significant digits) does over the same number of steps. For most
consumers this difference is invisible (both round to the same `f64` at
the point of use). But right after `dema`'s own warm-up completes, on
data that is still a perfectly straight, monotonic price ramp, the
`Decimal` version's iterative rounding produces a handful of *spurious*
sub-`1e-13`-magnitude sign flips in `short_dema - long_dema`, which the
strategy's exact `>`/`<=` crossover comparison reads as real trend
reversals — generating whipsaw entry/exit pairs on data that has no
actual reversal yet. The clean `f64` recomputation doesn't reproduce
that noise (it converges smoothly through the warmup instead), so those
spurious signals disappear and the test's hard-coded expectation of a
non-empty exit list breaks.

Tried and rejected: reordering the update to the algebraically-identical,
better-conditioned incremental form `prev + (val - prev) * k` (the same
kind of fix that worked for `smma.rs`, below) — this did not reproduce
the original's spurious oscillation either, confirming the divergence is
from `Decimal`'s extra internal precision itself, not from a poorly
conditioned `f64` formula.

This is arguably a case where the `f64` version is *more* correct (no
whipsaw on a monotonic trend), but per the hard gate here — preserve
behavior exactly, existing tests must pass unchanged — that's not this
fix's call to make. `ema.rs` was reverted to its original `Decimal`
implementation and excluded from this PR. It's used by 13 strategies
directly plus several other indicators (`dema`, `tema`, `macd`, `kama`,
...), so a change to its numerical behavior is high-blast-radius; revisiting
it would need either a formulation proven bit-identical to the current
output (as found for `smma.rs`) or a deliberate, reviewed decision to
accept the behavior change and update the affected test(s).

### `smma.rs`: numerically-stable reformulation required

A literal type-swap translation of `smma`'s update step
(`(prev * (period - 1) + val) / period`) failed
`test_smma_calculation` by 1 ULP (`12.444444444444443` vs the expected
`...445`) for the same reason as the `ema.rs` case above — per-step `f64`
rounding compounds differently than `Decimal`'s higher internal
precision. Unlike `ema.rs`, this one had a fix: the algebraically
identical but better-conditioned running-average form
`prev + (val - prev) / period` reproduces the `Decimal`-derived output
bit-for-bit (this is the standard numerically-stable incremental-mean
formula — it avoids scaling `prev` up by `period - 1` and back down,
which is where the extra rounding was introduced). All 3 `smma` tests
pass unchanged with this form; no test was modified.

### `wma.rs` was missed in the initial audit, converted separately

`wma.rs` matches the discarded-round-trip pattern (weighted rolling sum
over a `Decimal` window, `.to_f64()` on output) but was omitted from the
file list dispatched for conversion in this PR due to an audit oversight
— caught by a post-conversion `grep -rl rust_decimal` sweep over
`crates/strategies/src/indicators/` and fixed directly. Its 3 unit tests
pass unchanged.

See the PR for the full per-file diff.

Same harness, same fixture, same machine, same session:

```
I refs: 6,154,709,880   (this run's baseline: 6,793,524,674)
```

| | Ir | % of this run's baseline |
|---|---:|---:|
| Baseline (post-ATR-fix) | 6,793,524,674 | 100.00% |
| After strategy-layer fix | 6,154,709,880 | 90.60% |
| **Delta** | **-638,814,794** | **-9.40%** |

Clears the impact floor (≥5% instruction reduction) with room to spare.
`base2_to_decimal` self-cost drops from 2,589,703,962 (38.12%) to
2,024,588,995 (32.89%) — consistent with removing the price/ATR/output
round-trip from 47 files while deliberately leaving indicator-value
`Decimal` conversions (needed for `round_dp` display) and the two
`from_str`-based files untouched. `rust_decimal` internals still account
for the majority of instructions (~61%), split roughly evenly now between
the remaining indicator-value conversions in the 5 partially-converted
files, the 2 `from_str` files, `bollinger_bands.rs`'s variance
accumulator, and the indicator layer itself (`sma`, `ema`, `rsi`, etc. —
still `Decimal`-based per the original baseline note above). Each of
those is a separate, smaller, more case-by-case follow-up.

## Baseline for this run (before the backtest-loop O(n²) fix)

Recorded with `valgrind-3.22.0`, `perf/bolt_benchmark.sh --callgrind`,
release build of `thales-cli` (`--features nova`), same fixture, same
machine, same session, at the commit that includes the indicator-layer fix
above:

```
I refs: 2,623,791,528
```

```
Ir                    file:function
528,961,837 (20.16%)  rust_decimal::decimal::base2_to_decimal
240,536,843 ( 9.17%)  polars_core::chunked_array::ChunkedArray<T>::get
210,101,676 ( 8.01%)  rust_decimal::ops::common::Buf24::rescale
166,159,247 ( 6.33%)  rust_decimal::ops::div::div_impl
139,180,543 ( 5.30%)  thales_cli::backtest::run_backtest_with_strategy::{{closure}}
 93,809,950 ( 3.58%)  rust_decimal::ops::mul::mul_impl
 71,683,433 ( 2.73%)  __memcpy_avx_unaligned_erms
 66,201,049 ( 2.52%)  rust_decimal::ops::add::add_sub_internal
 64,434,900 ( 2.46%)  polars_core::chunked_array::builder::ChunkedBuilder::append_option
 60,016,571 ( 2.29%)  rust_decimal::ops::add::unaligned_add
```

`rust_decimal` internals remain the dominant cost (all attributable to the
13 indicators with a genuine precision reason, per the previous section).
The next entry that is a single, nameable function rather than diffuse
per-file cost is `polars_core::chunked_array::ChunkedArray<T>::get`
(9.17%) — but a `callgrind_annotate --tree=caller` walk shows that edge
fed by ~150 distinct call sites (every indicator and every strategy's
`generate_signals` closure), none contributing more than ~2% individually.
Fixing it would mean touching essentially every file that reads a polars
column, each with its own loop shape (some with backward-looking relative
indices) — not a single mechanism with a bounded, auditable diff, so it's
left as a finding rather than a fix (see PR notes).

The next candidate, `thales_cli::backtest::run_backtest_with_strategy::{{closure}}`
at **5.30%** self-cost, *is* a single function and clears the "worth
changing" bar (>5% of the profile). Reading `crates/cli/src/backtest.rs`'s
per-bar simulation loop shows why: on every bar (`for i in
0..bars.bars.len()`), the "Update Equity Curve" step recomputes

```rust
let realized_pnl: f64 = trades.iter().map(|t| t.pnl).sum();
```

— a full re-sum over *every trade closed so far* — to fold into the
equity-curve point for that single bar. `trades` only grows (trades are
never removed), so this is `O(bars × trades-closed-so-far)`, i.e.
quadratic in the length of the run for a strategy with a roughly constant
trade frequency, when a running accumulator updated at the two trade-close
sites (stop-loss/take-profit exit and signal-driven exit) would make it
`O(bars)`. At the fixture's scale (5,000 bars, ~tens of trades per
strategy) this doesn't yet dominate the profile the way `rust_decimal`
does, but it is a real algorithmic defect that gets worse, not better, as
users backtest over longer histories.

### Demonstrating the O(n²) shape before fixing it

A single fixed-size run can't distinguish "5.30% of a profile" from "5.30%
of a profile, and it doesn't scale" — for that we need the curve. Added
`perf/bolt_asymptotic.sh`, which drives the same public `backtest` entry
point with a single trade-heavy strategy (`EmaCrossover`, chosen because it
trades often enough on the synthetic random walk to make the
accumulated-`trades` effect visible) over three input sizes: the existing
5,000-bar fixture plus two new committed fixtures
(`perf/fixtures/synthetic_bars_10000.json`,
`..._20000.json`, generated the same way via
`thales-cli generate-synthetic-data --symbol SYNTH --initial-price 150.0
--timeframe 1h --num-bars <N>`).

Self-cost of `run_backtest_with_strategy::{{closure}}` alone, same
machine, same session, **at this commit** (fix not yet applied):

| Bars | Trades | Ir (closure self-cost) | % of run's total Ir |
|---:|---:|---:|---:|
| 5,000 | 124 | 1,239,653 | 1.27% |
| 10,000 | 222 | 3,600,788 | 1.86% |
| 20,000 | 479 | 11,729,958 | 2.97% |

Doubling the input roughly *triples* this function's self-cost each time
(2.96x, then 3.26x) — well past the ~2x a linear cost would produce, and
consistent with the `O(bars × trades)` mechanism read from the source:
trades grow roughly linearly with bars here, so the pure sum-recompute
term is quadratic in bars (would give 4x per doubling), and the observed
~3x is what that looks like once diluted by the other, genuinely linear
per-bar work already living in the same closure (equity-curve push,
signal-map lookup, position bookkeeping). This is the asymptotic-argument
evidence class (see project instructions): the fix is expected to flatten
this curve, not just shave a constant off it.
## After: backtest-loop realized-PnL accumulator

`crates/cli/src/backtest.rs`: replaced the per-bar `trades.iter().map(|t|
t.pnl).sum()` with a `realized_pnl: f64` accumulator initialized to `0.0`
before the simulation loop and incremented by `pnl` at each of the two
trade-close sites (intrabar SL/TP exit, and signal-driven exit) — the same
two places that already compute `pnl` for the trade being pushed. The
post-loop "Finalize Metrics" recomputation of the same sum was removed
too, reusing the accumulator instead (both are the same value, summed in
the same order, so this is bit-identical, not just equivalent). No
algorithm, ordering, or public output changed — this is a pure
`O(n²) -> O(n)` reduction of redundant work already being tracked
elsewhere.

Primary harness (`perf/bolt_benchmark.sh --callgrind`, same fixture, same
machine, same session):

```
I refs: 2,533,550,698   (this run's baseline: 2,623,791,528)
```

| | Ir | % of this run's baseline |
|---|---:|---:|
| Baseline (post-indicator-layer-fix) | 2,623,791,528 | 100.00% |
| After backtest-loop fix | 2,533,550,698 | 96.56% |
| **Delta** | **-90,240,830** | **-3.44%** |

`run_backtest_with_strategy::{{closure}}` self-cost drops from
139,180,543 (5.30%) to 51,106,946 (2.02%) — a 63% reduction in the target
function itself. The whole-workload delta (-3.44%) does not on its own
clear the flat "≥5% of total instructions" bar, so this change is
justified instead by the asymptotic-improvement criterion: same three
input sizes, same `perf/bolt_asymptotic.sh` harness, **after** the fix:

| Bars | Trades | Ir (closure self-cost) | % of run's total Ir |
|---:|---:|---:|---:|
| 5,000 | 124 | 547,140 | 0.57% |
| 10,000 | 222 | 1,258,253 | 0.66% |
| 20,000 | 479 | 2,141,407 | 0.56% |

Before the fix this function's cost share grew with input size (1.27% ->
1.86% -> 2.97%); after the fix it is flat (~0.55-0.66%, no growth trend) —
the quadratic term is gone, leaving the genuinely linear per-bar work
(equity-curve push, signal-map lookup, position bookkeeping). This is the
"asymptotic complexity improvement, demonstrated across input sizes" gate,
independent of the whole-workload percentage, which is small only because
`rust_decimal` still dominates the total at this fixture size (see
previous sections) — the defect this fixes gets proportionally worse, not
better, on longer backtests, which is exactly the scenario a real user
runs when evaluating a strategy over years of history rather than a few
thousand hourly bars.

`cargo test --workspace --all-features` (924 unit + doctests across the
workspace) passes unchanged; no test's expected trade count, PnL, or
equity value needed updating, consistent with this being a pure
redundant-work removal rather than a behavior change.

### Reproduce

```sh
cargo build --release -p thales-cli --features nova
perf/bolt_benchmark.sh --callgrind     # whole-workload delta
perf/bolt_asymptotic.sh --callgrind    # per-size scaling curve
```
