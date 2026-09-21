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
