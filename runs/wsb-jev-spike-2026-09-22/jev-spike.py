#!/usr/bin/env python3
"""Spike: score WSB sentiment for NVDA with a Jev Score+Noul turn.

Reads runs/<run-id>/wsb-NVDA-raw.json, asks Jev four typed questions in one
round trip (lean/conviction/euphoria/surface), and writes the full turn
(state + questions + answers + usage) to wsb-NVDA-jev.json for the audit
trail. Exit 0 on a scored turn, 1 if Jev is unreachable (fail closed —
the deterministic keyword score stands as the evidence).
"""
import json
import subprocess
import sys
from datetime import datetime, timezone

RUN_DIR = "/home/hatch/workspace/thales/runs/wsb-jev-spike-2026-09-22"
JEV_PY = "/home/hatch/workspace/skills/typesafe/bin/jev.py"


def main() -> int:
    with open(f"{RUN_DIR}/wsb-NVDA-raw.json") as f:
        raw = json.load(f)

    sources = [
        {
            "title": r.get("title", ""),
            "snippet": r.get("snippet", ""),
            "url": r.get("url", ""),
            "date": r.get("date", ""),
        }
        for r in raw.get("results", [])
    ]

    state = {
        "task": "wsb_sentiment_spike",
        "ticker": raw.get("symbol", "NVDA"),
        "asof": datetime.now(timezone.utc).astimezone().isoformat(),
        "context": (
            "You are scoring WallStreetBets retail sentiment as a CONTRARIAN "
            "mania meter for a paper-trading scan. WSB speaks in irony, "
            "sarcasm, and gallows humor — read tone, not just keywords. "
            "Euphoria means crowded-trade caution, never a buy signal. "
            "Some sources are stale-dated; weigh recency. Answer only from "
            "the sources below."
        ),
        "sources": sources,
    }

    questions = {
        "lean": {
            "type": "choice",
            "instructions": "What is the directional lean of WSB chatter on NVDA across these sources?",
            "criteria": {
                "bullish": "Chatter clearly leans bullish: more optimism, upside targets, and call-side energy than fear.",
                "bearish": "Chatter clearly leans bearish: more fear, downside talk, and put-side hedging than optimism.",
                "mixed": "Genuinely two-sided: bullish and bearish voices both present, no clear majority.",
                "quiet": "Too little real chatter to call a lean: stale, off-topic, or thin results.",
            },
        },
        "conviction": {
            "type": "score",
            "instructions": "How intense is the WSB attention on NVDA right now?",
            "criteria": [
                "No meaningful attention: quiet or stale results only",
                "Light chatter: routine mentions, no excitement",
                "Moderate buzz: elevated mentions, real debate",
                "Heavy buzz: surging mentions, strong directional energy",
                "Euphoric pile-on: mania-like, meme-stock energy",
            ],
        },
        "euphoria": {
            "type": "noul",
            "instructions": "Is WSB euphoric about NVDA — crowded-trade territory?",
            "criteria": {
                "true": "Euphoric: pile-on, moon-talk, mania-like. Read as crowded-trade caution.",
                "false": "Not euphoric: measured, mixed, or quiet chatter.",
            },
        },
        "surface": {
            "type": "noul",
            "instructions": "Is this sentiment development worth surfacing to Mark in the morning briefing?",
            "criteria": {
                "true": "Yes: new, strong, or surprising enough to earn a line in the brief.",
                "false": "No: routine or thin. Keep it in the audit file only.",
            },
        },
    }

    proc = subprocess.run(
        [sys.executable, JEV_PY, "ask",
         "--state", json.dumps(state),
         "--questions", json.dumps(questions)],
        capture_output=True, text=True, timeout=120,
    )
    try:
        report = json.loads(proc.stdout)
    except json.JSONDecodeError:
        report = {"ok": False, "parse_error": proc.stdout[-500:], "stderr": proc.stderr[-500:]}

    out = {
        "spike": "wsb-jev-spike-2026-09-22",
        "symbol": raw.get("symbol", "NVDA"),
        "scored_at": datetime.now(timezone.utc).astimezone().isoformat(),
        "state": state,
        "questions": questions,
        "jev_report": report,
    }
    with open(f"{RUN_DIR}/wsb-NVDA-jev.json", "w") as f:
        json.dump(out, f, indent=2)

    if not report.get("ok"):
        print(json.dumps({"ok": False, "error": report}, indent=2)[:2000])
        return 1
    print(json.dumps(report.get("answers", {}), indent=2))
    print(json.dumps(report.get("usage", {}), indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
