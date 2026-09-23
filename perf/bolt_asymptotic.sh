#!/usr/bin/env bash
# Asymptotic-scaling harness for the backtest simulation loop.
#
# Drives the CLI's public `backtest` entry point (crates/cli/src/backtest.rs)
# with a single, trade-heavy strategy (EmaCrossover) over bar series of
# increasing length, to demonstrate algorithmic-complexity changes in
# run_backtest_with_strategy that a single fixed-size run can't show: a
# per-bar cost that scales with the number of trades accumulated so far
# looks like noise at one input size but shows up as a growing share of
# total instructions as the input grows.
#
# Usage:
#   perf/bolt_asymptotic.sh            # wall-clock (context only, not admissible evidence)
#   perf/bolt_asymptotic.sh --callgrind  # instruction counts via valgrind --tool=callgrind
#
# Requires a release build: cargo build --release -p thales-cli --features nova
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

BIN=target/release/thales-cli
STRATEGY=EmaCrossover
SIZES=(5000 10000 20000)

if [ ! -x "$BIN" ]; then
  echo "error: $BIN not found. Build it first:" >&2
  echo "  cargo build --release -p thales-cli --features nova" >&2
  exit 1
fi

for N in "${SIZES[@]}"; do
  FIXTURE="perf/fixtures/synthetic_bars_${N}.json"
  if [ ! -f "$FIXTURE" ]; then
    echo "error: $FIXTURE not found" >&2
    exit 1
  fi

  if [ "${1:-}" = "--callgrind" ]; then
    OUT="perf/callgrind.asymptotic.${N}.out"
    valgrind --tool=callgrind --callgrind-out-file="$OUT" \
      "$BIN" backtest --input "$FIXTURE" --strategy "$STRATEGY" \
        --initial-capital 10000 --risk 100 \
      > /dev/null
    echo "=== N=$N ==="
    echo "Wrote $OUT"
    callgrind_annotate --threshold=100 "$OUT" 2>/dev/null | grep -E "PROGRAM TOTALS|run_backtest_with_strategy" || true
  else
    echo "=== N=$N ==="
    time "$BIN" backtest --input "$FIXTURE" --strategy "$STRATEGY" \
      --initial-capital 10000 --risk 100 > /dev/null
  fi
done
