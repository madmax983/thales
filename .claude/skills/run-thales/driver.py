#!/usr/bin/env python3
"""Drive the Thales CLI end to end without a single API key.

Subcommands:
  smoke   full pipeline: paper bars -> backtest -> signals -> execute
  judge   judge-signals against a local mock System One (no TYPESAFE_API_KEY)
  probe   run a Rust snippet against the workspace crates (direct invocation)
  ci      the four gates CI enforces

Everything lands in --out (default: target/thales-run). Run from the repo root.
"""

import argparse
import json
import os
import pathlib
import shutil
import subprocess
import sys
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer

ROOT = pathlib.Path(__file__).resolve().parents[3]
BIN = ROOT / "target" / "debug" / "thales-cli"


def sh(cmd, **kw):
    print(f"$ {' '.join(str(c) for c in cmd)}", file=sys.stderr)
    return subprocess.run(cmd, cwd=ROOT, **kw)


def build():
    if sh(["cargo", "build", "-p", "thales-cli"]).returncode != 0:
        sys.exit("build failed")


def cli(args, out_path=None, env=None):
    """Run the CLI, parse the JSON envelope, fail loudly on status != ok.

    stderr is left attached to the terminal on purpose: several commands
    narrate their sizing decisions there and that narration is the useful
    part when a signal comes back empty.
    """
    e = dict(os.environ)
    e.update(env or {})
    p = subprocess.run([str(BIN), *args], cwd=ROOT, env=e, capture_output=True, text=True)
    sys.stderr.write(p.stderr)
    try:
        env_json = json.loads(p.stdout)
    except json.JSONDecodeError:
        sys.exit(f"non-JSON stdout from {args[0]} (exit {p.returncode}):\n{p.stdout[:800]}")
    if env_json.get("status") != "ok":
        sys.exit(f"{args[0]} returned errors: {env_json.get('errors')}")
    if env_json.get("warnings"):
        print(f"  warnings: {env_json['warnings']}", file=sys.stderr)
    if out_path:
        out_path.write_text(p.stdout)
    return env_json["data"]


# --- mock System One -------------------------------------------------------
# judge-signals refuses to run without TYPESAFE_API_KEY and will not pass
# signals through ungated, so the only offline way to exercise the gate is to
# stand up something that answers POST /v1/systemone. The answer shape below
# is the one crates/providers/jev expects; the thresholds it gets compared
# against are the CLI's --min-* flags.
def answer_body(verdict, p, confidence, quality):
    rest = (1.0 - p) / 2.0
    return {
        "model": "jev-mock",
        "answers": {
            "verdict": {
                "type": "choice",
                "choice": verdict,
                "confidence": confidence,
                "probabilities": {
                    k: (p if k == verdict else rest)
                    for k in ("execute", "reduce_size", "skip")
                },
            },
            "instrument_quality": {"type": "noul", "noul": quality},
            "regime_fit": {"type": "noul", "noul": 0.82},
            "conviction": {
                "type": "score",
                "score": 3.0,
                "confidence": 0.9,
                "legend": {"0": "No edge", "1": "Weak", "2": "Fair", "3": "Strong", "4": "Exceptional"},
                "probabilities": {"0": 0.0, "1": 0.1, "2": 0.2, "3": 0.6, "4": 0.1},
            },
        },
        "usage": {"input_tokens": 410, "output_tokens": 14},
    }


class MockSystemOne(HTTPServer):
    allow_reuse_address = True


def start_mock(verdict, p, confidence, quality):
    payload = json.dumps(answer_body(verdict, p, confidence, quality)).encode()

    class H(BaseHTTPRequestHandler):
        def do_POST(self):
            n = int(self.headers.get("content-length", 0))
            self.rfile.read(n)
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def log_message(self, *a):
            pass

    srv = MockSystemOne(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, f"http://127.0.0.1:{srv.server_address[1]}"


# --- subcommands -----------------------------------------------------------
def cmd_smoke(a):
    build()
    out = pathlib.Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    bars, bt, sig = out / "bars.json", out / "backtest.json", out / "signals.json"

    # `paper` is the only provider that needs no keys; it synthesises bars that
    # reliably trigger BollingerBands, which is why the smoke path uses it.
    d = cli(["fetch-market-data", "--provider", "paper", "--symbol", a.symbol,
             "--timeframe", "1h"], bars)
    n = len(d["bars"])
    assert n > 20, f"only {n} bars"
    print(f"bars: {n}")

    m = cli(["backtest", "--input", str(bars), "--strategy", a.strategy,
             "--initial-capital", "10000", "--risk", "100"], bt)["metrics"]
    print(f"backtest: {m['total_trades']} trades, {m['total_return_pct']:.2f}% return, "
          f"{m['max_drawdown_pct']:.2f}% max DD")

    s = cli(["generate-signals", "--input", str(bars), "--strategy", a.strategy], sig)
    print(f"signals: {len(s)}")
    if not s:
        # Documented behaviour, not a failure: the strategy only emits on the
        # final candle. Stop here rather than feeding [] into the executor.
        print("no signal on the last candle - nothing to execute", file=sys.stderr)
        return

    fills = cli(["execute-intent", "--provider", "paper", "--input", str(sig)])
    for f in fills:
        print(f"fill: {f['symbol'] if 'symbol' in f else f['intent_id']} -> "
              f"{f['status']} ({f['provider_order_id']})")
    assert all(f["status"] == "filled" for f in fills)
    print(f"\nOK - artifacts in {out}")


def cmd_judge(a):
    build()
    out = pathlib.Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    bars, sig, judged = out / "bars.json", out / "signals.json", out / "judged.json"

    # Regenerate when the cached signals are missing OR empty: a previous
    # `smoke --strategy X` may have left a legitimate `[]` behind, and judging
    # nothing is a confusing no-op rather than a useful run.
    cached = json.loads(sig.read_text())["data"] if sig.exists() else []
    stale = not cached or any(x.get("strategy") != a.strategy for x in cached)
    if stale:
        if not bars.exists():
            cli(["fetch-market-data", "--provider", "paper", "--symbol", a.symbol,
                 "--timeframe", "1h"], bars)
        cli(["generate-signals", "--input", str(bars), "--strategy", a.strategy], sig)
    before = json.loads(sig.read_text())["data"]
    if not before:
        sys.exit(f"{a.strategy} did not fire on the last candle, so there is "
                 f"nothing to judge. Try --strategy BollingerBands, which the "
                 f"paper bars reliably trigger.")

    srv, url = start_mock(a.verdict, a.p, a.confidence, a.quality)
    try:
        args = ["judge-signals", "--input", str(sig), "--bars", str(bars)]
        if a.emit:
            args += ["--emit", a.emit]
        data = cli(args, judged, env={"TYPESAFE_API_KEY": "sk-mock",
                                      "TYPESAFE_BASE_URL": url})
    finally:
        srv.shutdown()

    if a.emit == "report":
        print(json.dumps(data, indent=2)[:2000])
        return
    print(f"in: {len(before)} signal(s), out: {len(data)} - verdict '{a.verdict}'")
    for b in before:
        match = next((x for x in data if x["intent_id"] == b["intent_id"]), None)
        if match is None:
            print(f"  {b['intent_id']}: REJECTED")
        else:
            print(f"  {b['intent_id']}: confidence {b['confidence']} -> {match['confidence']}, "
                  f"size {b['size_hint']} -> {match['size_hint']}")
            print(f"    {match['rationale'].split('| jev')[-1].strip()}")


PROBE_DIR = ROOT / "crates" / "strategies" / "examples"
PROBE = PROBE_DIR / "_probe.rs"


def cmd_probe(a):
    """Call workspace internals directly - the layer most PRs actually touch.

    Writes a throwaway example into the strategies crate, runs it, deletes it.
    It MUST be deleted: `cargo clippy --all-targets` compiles examples, so a
    leftover probe turns into a CI failure in an untracked file.
    """
    src = pathlib.Path(a.file).read_text()
    if "fn main" not in src:
        src = ("use polars::prelude::*;\n#[allow(unused_imports)]\nuse strategies::*;\n"
               "fn main() -> anyhow::Result<()> {\n" + src + "\n    Ok(())\n}\n")
    PROBE_DIR.mkdir(parents=True, exist_ok=True)
    PROBE.write_text(src)
    try:
        r = sh(["cargo", "run", "-q", "-p", a.crate, "--example", "_probe"])
    finally:
        PROBE.unlink(missing_ok=True)
        if PROBE_DIR.exists() and not any(PROBE_DIR.iterdir()):
            shutil.rmtree(PROBE_DIR)
    sys.exit(r.returncode)


def cmd_ci(a):
    gates = [
        ["cargo", "fmt", "--all", "--check"],
        ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
        ["cargo", "test", "--workspace"],
        ["cargo", "check", "-p", "thales-cli", "--features", "nova", "--all-targets"],
    ]
    bad = [g for g in gates if sh(g).returncode != 0]
    if bad:
        sys.exit(f"{len(bad)} gate(s) failed: " + "; ".join(" ".join(g) for g in bad))
    print("all four CI gates pass")


p = argparse.ArgumentParser(description=__doc__,
                            formatter_class=argparse.RawDescriptionHelpFormatter)
p.add_argument("--out", default="target/thales-run")
sub = p.add_subparsers(dest="cmd", required=True)

sp = sub.add_parser("smoke", help="paper bars -> backtest -> signals -> paper fill")
sp.add_argument("--strategy", default="BollingerBands")
sp.add_argument("--symbol", default="BTCUSD")
sp.set_defaults(fn=cmd_smoke)

jp = sub.add_parser("judge", help="judge-signals against a local mock System One")
jp.add_argument("--verdict", default="execute", choices=["execute", "reduce_size", "skip"])
jp.add_argument("--p", type=float, default=0.9, help="probability of the chosen verdict")
jp.add_argument("--confidence", type=float, default=0.95)
jp.add_argument("--quality", type=float, default=0.9, help="instrument_quality noul")
jp.add_argument("--emit", choices=["intents", "report"])
jp.add_argument("--strategy", default="BollingerBands")
jp.add_argument("--symbol", default="BTCUSD")
jp.set_defaults(fn=cmd_judge)

pp = sub.add_parser("probe", help="run a Rust snippet against workspace internals")
pp.add_argument("file")
pp.add_argument("--crate", default="strategies")
pp.set_defaults(fn=cmd_probe)

cp = sub.add_parser("ci", help="fmt + clippy + test + nova")
cp.set_defaults(fn=cmd_ci)

a = p.parse_args()
a.fn(a)
