import execute_cycle
import json

candidate = {"provider": "paper", "symbol": "BTCUSD", "market": "crypto"}
strategies = execute_cycle.get_active_strategies()
signals = execute_cycle.evaluate_candidate(candidate, strategies)
print(f"Generated signals sides: {[s['side'] for s in signals]}")
resolved = execute_cycle.resolve_conflicts(signals)
print(f"Resolved signals length: {len(resolved)}")
