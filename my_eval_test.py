import execute_cycle
import json

candidate = {"provider": "paper", "symbol": "BTCUSD", "market": "crypto"}
strategies = execute_cycle.get_active_strategies()
print(f"Strategies: {strategies}")

bars = execute_cycle.run_command(["fetch-market-data", "--provider", "paper", "--symbol", "BTCUSD", "--timeframe", "1h"])
print(f"Bars fetched: {len(bars.get('bars', [])) if isinstance(bars, dict) else 'No bars'}")

signals = execute_cycle.evaluate_candidate(candidate, strategies)
print(f"Signals generated: {len(signals)}")
if signals:
    print(json.dumps(signals[0], indent=2))
