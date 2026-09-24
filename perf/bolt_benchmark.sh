#!/usr/bin/env bash
# Realistic-workload harness for performance work on thales-cli.
#
# Exercises the public `benchmark` entry point (crates/cli/src/benchmark.rs),
# which runs every registered strategy's `generate_signals` + backtest loop
# over a bar series -- this is the heaviest CPU path a user of the CLI
# actually triggers (`thales-cli benchmark --input <bars.json>`).
#
# Usage:
#   perf/bolt_benchmark.sh            # wall-clock run (context only, not admissible evidence)
#   perf/bolt_benchmark.sh --callgrind  # instruction-count profile via valgrind --tool=callgrind
#   perf/bolt_benchmark.sh --dhat       # allocation count/bytes profile via valgrind --tool=dhat
#
# Requires a release build: cargo build --release -p thales-cli --features nova
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

BIN=target/release/thales-cli
FIXTURE=perf/fixtures/synthetic_bars_5000.json

if [ ! -x "$BIN" ]; then
  echo "error: $BIN not found. Build it first:" >&2
  echo "  cargo build --release -p thales-cli --features nova" >&2
  exit 1
fi

if [ "${1:-}" = "--callgrind" ]; then
  OUT=perf/callgrind.benchmark.out
  valgrind --tool=callgrind --callgrind-out-file="$OUT" \
    "$BIN" benchmark --input "$FIXTURE" --initial-capital 10000 --risk 100 --sort-by total_return \
    > /dev/null
  echo "Wrote $OUT"
  echo "Top self-cost functions:"
  callgrind_annotate --threshold=80 "$OUT" 2>/dev/null | sed -n '1,40p'
elif [ "${1:-}" = "--dhat" ]; then
  OUT=perf/dhat.benchmark.json
  valgrind --tool=dhat --dhat-out-file="$OUT" \
    "$BIN" benchmark --input "$FIXTURE" --initial-capital 10000 --risk 100 --sort-by total_return \
    > /dev/null
  echo "Wrote $OUT"
else
  time "$BIN" benchmark --input "$FIXTURE" --initial-capital 10000 --risk 100 --sort-by total_return > /dev/null
fi
