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
