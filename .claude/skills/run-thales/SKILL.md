---
name: run-thales
description: Build, run, and drive the Thales trading CLI - fetch bars, backtest, generate signals, judge them through the Jev gate, and execute on paper, all without API keys. Use when asked to run, start, build, test, smoke-test, or screenshot Thales, to confirm a strategy/indicator change works in the real binary, or to reproduce the CI gates locally.
---

# Running Thales

Thales is a **JSON-first Rust CLI** (`thales-cli`), not a server or a GUI. Every
command reads files and writes a `{"status","errors","warnings","data"}` envelope
to stdout. There is nothing to screenshot; the observable surface is JSON.

The agent path is **`.claude/skills/run-thales/driver.py`**, which builds the
binary and drives the real pipeline end to end with **no API keys** - including
`judge-signals`, which normally refuses to run without a paid TypeSafe key.

All paths below are relative to the repo root (`/Users/mark/thales`).

## Prerequisites

Rust stable and Python 3 (stdlib only - the driver's mock server is
`http.server`). Nothing else. No `apt-get`, no Docker, no services.

```bash
rustup update stable
cargo build -p thales-cli     # ~22s cold, polars dominates
```

## Run (agent path)

### Full pipeline, no keys

```bash
python3 .claude/skills/run-thales/driver.py smoke
```

Builds, then: `fetch-market-data --provider paper` -> `backtest` ->
`generate-signals` -> `execute-intent --provider paper`. It asserts every
envelope is `status: ok` and every fill is `filled`, and leaves the JSON in
`target/thales-run/`. Verified output:

```
bars: 100
backtest: 5 trades, 2.19% return, 0.57% max DD
signals: 1
fill: crypto:BTCUSD:buy:1790090126412 -> filled (paper-BTCUSD-1790090126629)
```

Other strategies (83 are registered in `crates/cli/src/strategy_factory.rs`):

```bash
python3 .claude/skills/run-thales/driver.py smoke --strategy KamaRsiTrend
```

### The Jev gate, without a TypeSafe key

`judge-signals` hard-fails on a missing `TYPESAFE_API_KEY` - by design, it will
never pass signals through ungated. The driver stands up a local mock System One
on a random port, points `TYPESAFE_BASE_URL` at it, and serves whatever verdict
you ask for:

```bash
python3 .claude/skills/run-thales/driver.py judge                       # approve
python3 .claude/skills/run-thales/driver.py judge --verdict reduce_size # halve size
python3 .claude/skills/run-thales/driver.py judge --verdict skip        # reject
python3 .claude/skills/run-thales/driver.py judge --quality 0.2         # quality veto
python3 .claude/skills/run-thales/driver.py judge --emit report         # full audit trail
```

Verified, in order - this is the whole decision table from `README.md` exercised
offline:

```
in: 1 signal(s), out: 1 - verdict 'execute'
  ...: confidence 0.64 -> 0.9, size 0.037850 -> 0.037850
in: 1 signal(s), out: 1 - verdict 'reduce_size'
  ...: confidence 0.64 -> 0.05, size 0.037850 -> 0.018925
in: 1 signal(s), out: 0 - verdict 'skip'
  ...: REJECTED
in: 1 signal(s), out: 0 - verdict 'execute'      # rejected on instrument quality
```

`judge` reuses `target/thales-run/signals.json` when it holds a signal and
regenerates it otherwise, so it works standalone - no need to run `smoke` first.
Pass `--strategy` to judge a different strategy's output.

### Direct invocation of internals

Most PRs here touch one function in `crates/strategies/src/indicators/` - the
last one replaced `Decimal` with `f64` inside `atr.rs`. Going through the CLI to
check a change like that is the long way round. `probe` runs a Rust snippet
against the workspace crates instead:

```bash
cat > /tmp/atr.rs <<'EOF'
    let df = df![
        "high"  => [10.0f64, 11.0, 12.0, 11.5, 13.0, 12.5, 14.0],
        "low"   => [ 9.0f64,  9.5, 10.5, 10.0, 11.0, 11.5, 12.0],
        "close" => [ 9.5f64, 10.5, 11.0, 10.5, 12.5, 12.0, 13.5],
    ]?;
    println!("{:?}", strategies::indicators::atr::calculate(&df, 3)?);
EOF
python3 .claude/skills/run-thales/driver.py probe /tmp/atr.rs
```

```
shape: (7,)
Series: 'atr' [f64]
[null, null, 1.333333, 1.388889, 1.759259, 1.506173, 1.670782]
```

A bare snippet is wrapped in `fn main() -> anyhow::Result<()>` with
`polars::prelude::*` and `strategies::*` in scope, so `?` and `df!` just work.
Provide your own `fn main` and it is used verbatim. `--crate contracts` (or
`thales-cli`) targets a different crate. The driver writes
`crates/strategies/examples/_probe.rs`, runs it, and **always deletes it** - see
Gotchas.

### CI gates

```bash
python3 .claude/skills/run-thales/driver.py ci
```

Runs the same four gates `.github/workflows/ci.yml` enforces, in order, and
names the ones that failed. All four pass on `trunk` as of this writing:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                                       # 24 doctests + unit/integration
cargo check -p thales-cli --features nova --all-targets
```

Merges are gated on this, so run it before opening a PR.

## Run (human path)

Straight `cargo run`, one stage at a time. Useful when you want to vary a flag
the driver does not expose:

```bash
cargo run -p thales-cli -- fetch-market-data --provider paper \
  --symbol BTCUSD --timeframe 1h > bars.json
cargo run -p thales-cli -- backtest --input bars.json \
  --strategy BollingerBands --initial-capital 10000 --risk 100
```

`--provider kraken` / `alpaca` need real keys and will fail cleanly without them:

```json
{"status":"error","errors":["provider error: missing required environment variable: KRAKEN_API_KEY"],"warnings":[],"data":null}
```

**`--provider kraken` on `execute-intent` places a live order.** Use `paper`.

## Performance work

A harness already exists - don't write another. It profiles `benchmark`, the
heaviest CPU path (every strategy x the full bar series):

```bash
cargo build --release -p thales-cli --features nova
perf/bolt_benchmark.sh              # wall clock (~1.2s on the 5000-bar fixture)
perf/bolt_benchmark.sh --callgrind  # instruction counts; needs valgrind
```

Wall-clock numbers are context only - `perf/README.md` treats instruction counts
as the admissible evidence.

## Gotchas

- **`--features nova` clobbers the default binary.** Both builds write
  `target/debug/thales-cli`. Building with `nova` swaps in a binary with 42
  extra subcommands (20 -> 62); building without swaps it back, from cache, in
  0.1s, with no recompilation and no output saying anything changed. If a
  command you know exists reports "unrecognized subcommand", you are holding
  the other build.
  The driver always rebuilds default-features first.
- **A leftover `_probe.rs` breaks CI.** `cargo clippy --all-targets` compiles
  examples, `crates/strategies/examples/` is not gitignored, and a scratch
  example with a warning fails `-D warnings` in a file you never meant to
  commit. The driver deletes it in a `finally` - including after a compile
  error - and removes the directory if it emptied. If you hand-roll a probe,
  do the same.
- **Empty `generate-signals` output is normal, not a bug.** Strategies emit only
  if they trigger on the *last* candle. `--provider paper` synthesises bars that
  reliably trigger `BollingerBands`, which is why the driver uses it; a real
  provider's data often yields `[]`. The driver stops there rather than feeding
  `[]` to the executor.
- **`generate-signals` narrates to stderr, not stdout.** `Risk-based Sizing: ...`
  and `DEBUG: No position for ...` are stderr; stdout stays clean JSON. Don't
  `2>&1` into a parser - and don't ignore the stream either, it is where the
  sizing arithmetic is explained.
- **On a `reduce_size` verdict, `confidence` becomes the probability of
  `execute`, not of the chosen verdict.** The mock returns `execute` at 0.05
  while `reduce_size` carries 0.90, and the surviving intent reports
  `confidence: 0.05` with `size_hint` halved by `--reduce-factor`. Looks wrong
  at a glance; it is the gate reporting calibrated execute-probability.
- **`analyze-market` dirties your working tree.** It looks read-only and its
  name says analysis, but unless you pass `--no-report` it *appends* to four
  tracked files in the repo root - `Signals.md`, `Market_Regime.md`,
  `Volatility_Regime.md`, `Market_Research.md` (`crates/cli/src/main.rs:1177`).
  Run it, then `git status`, and you have four modified files you did not
  intend to commit. Always `--no-report` unless you specifically want the
  report appended. `git checkout Signals.md Market_Regime.md
  Volatility_Regime.md Market_Research.md` undoes it. `generate-signals` and
  `backtest` do not do this.
- **The gate can only remove or shrink.** It never invents a signal and never
  raises `size_hint`. If the count goes up, something else is wrong.
- **Paper bars are deterministic in price but not in time.** Two consecutive
  `fetch-market-data --provider paper` runs produce byte-identical OHLC - so
  `backtest` reprints `5 trades, 2.1915%` exactly, and a metrics change really
  is your change. Only `timestamp_unix_ms` moves (it is anchored to the clock),
  and since `intent_id` embeds that timestamp, *every* generated signal gets a
  fresh id. Diff intents on `symbol`/`side`/`size_hint`, never on `intent_id`.

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| `error: unrecognized subcommand 'market-weather'` | Nova build got swapped out. `cargo build -p thales-cli --features nova`. |
| `{"status":"error",...,"missing TYPESAFE_API_KEY"}` | Use `driver.py judge`; it supplies a mock key and base URL. Real keys are only needed to hit the real model. |
| `missing required environment variable: KRAKEN_API_KEY` | Expected. Use `--provider paper`. |
| `<Strategy> did not fire on the last candle, so there is nothing to judge` | That strategy had no setup on the final bar. Use `--strategy BollingerBands`. |
| `clippy` fails in `examples/_probe.rs` | Stale probe. `rm -rf crates/strategies/examples`. |
| `non-JSON stdout from <cmd>` | The command panicked or printed plain text (some nova commands print report text, not envelopes). Read the captured output the driver prints. |
| `git status` shows modified `Signals.md` / `Market_Regime.md` / `Volatility_Regime.md` / `Market_Research.md` | You ran `analyze-market` without `--no-report`. `git checkout` those four. |
| `error: target/release/thales-cli not found` from `bolt_benchmark.sh` | It needs a **release** build with nova: `cargo build --release -p thales-cli --features nova`. |
