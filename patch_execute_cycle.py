import re

with open("execute_cycle.py", "r") as f:
    content = f.read()

# Fix get_candidates_from_signals
new_get_candidates = """
        # Check for Staleness (24 hours = 86400000 ms)
        signal_ref = "NO_REF"
        expected_side = None
        if raw_json and raw_json.get("timestamp_unix_ms"):
            ts = raw_json["timestamp_unix_ms"]
            rec = raw_json.get("recommendation", "").lower()
            if "short" in rec or "sell" in rec: expected_side = "sell"
            elif "long" in rec or "buy" in rec: expected_side = "buy"

            side_str = expected_side if expected_side else "unknown"
            signal_ref = f"{market}:{symbol}:{side_str}:{ts}"

            now = int(datetime.now().timestamp() * 1000)
            if (now - ts) > 86400000:
                 age_hours = (now - ts) / 3600000
                 reason = f"Signal too old ({age_hours:.1f} hours > 24 hours)"
                 print(f"Skipping stale signal for {symbol}: {reason}")

                 # Log to portfolio.md
                 dummy_intent = {
                     "symbol": symbol,
                     "intent_id": signal_ref,
                     "rationale": "Stale signal from Signals.md"
                 }
                 log_skipped(dummy_intent, reason)
                 continue
"""

content = re.sub(
    r'        # Check for Staleness.*?log_skipped\(dummy_intent, reason\)\n                 continue',
    new_get_candidates.strip(),
    content,
    flags=re.DOTALL
)

# Add signal_ref and expected_side to candidate dict
content = re.sub(
    r'            "market": market,\n            "raw_analysis_json": raw_json\n        \}',
    '            "market": market,\n            "raw_analysis_json": raw_json,\n            "expected_side": expected_side,\n            "signal_ref": signal_ref\n        }',
    content
)

# Fix resolve_conflicts
new_resolve_conflicts = """
def resolve_conflicts(intents, candidate):
    \"\"\"
    Resolves conflicts among signals for the same candidate.
    - Filters out intents that conflict with Signals.md expected side.
    - If strategies conflict (Buy vs Sell), strictly REJECTS execution for that asset.
    - Returns single best intent.
    \"\"\"
    if not intents:
        return []

    symbol = candidate["symbol"]
    signal_ref = candidate.get("signal_ref", "NO_REF")
    expected_side = candidate.get("expected_side")

    if expected_side:
        valid_intents = [i for i in intents if i["side"] == expected_side]
        if not valid_intents:
            invalid_sides = set(i["side"] for i in intents)
            reason = f"Cross-validation failed: Strategies generated {invalid_sides} but signal recommended {expected_side}."
            print(f"  {symbol}: {reason}")
            dummy_intent = {"symbol": symbol, "intent_id": signal_ref}
            log_skipped(dummy_intent, reason)
            return []
        intents = valid_intents

    sides = set(intent["side"] for intent in intents)
    if len(sides) > 1:
        reason = f"Conflict: Active strategies generated conflicting signals ({sides}) for {symbol}."
        print(f"  {symbol}: {reason}")
        dummy_intent = {"symbol": symbol, "intent_id": signal_ref}
        log_skipped(dummy_intent, reason)
        return []

    analysis = intents[0].get("_market_analysis")
    def score_intent(intent):
        confidence = float(intent.get("confidence", 0.0) or 0.0)
        strategy_name = intent.get("strategy", "")
        return confidence * strategy_regime_weight(strategy_name, analysis)

    intents.sort(
        key=lambda x: (score_intent(x), float(x.get("confidence", 0.0) or 0.0)),
        reverse=True,
    )
    best_intent = intents[0]

    if signal_ref != "NO_REF":
        best_intent["intent_id"] = signal_ref

    return [best_intent]
"""

content = re.sub(
    r'def resolve_conflicts\(intents, conflict_margin=0\.05\):.*?return \[best_intent\]',
    new_resolve_conflicts.strip(),
    content,
    flags=re.DOTALL
)

# Update the call to resolve_conflicts in main()
content = content.replace(
    'valid_signals = resolve_conflicts(raw_signals)',
    'valid_signals = resolve_conflicts(raw_signals, cand)'
)

# Update the log_skipped call for NO_STRATEGY_SIGNAL to use candidate signal_ref
content = content.replace(
    '"intent_id": "NO_STRATEGY_SIGNAL",',
    '"intent_id": cand.get("signal_ref", "NO_STRATEGY_SIGNAL"),'
)

with open("execute_cycle.py", "w") as f:
    f.write(content)
