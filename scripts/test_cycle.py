import sys
import os
import json
import unittest
from unittest.mock import MagicMock, patch

# Add root to sys.path
sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

import execute_cycle

class TestCycle(unittest.TestCase):
    def setUp(self):
        self.history_file = "test_history.json"
        if os.path.exists(self.history_file):
            os.remove(self.history_file)

        # Patch constants
        self.original_history_path = execute_cycle.HISTORY_PATH
        self.original_cli_path = execute_cycle.CLI_PATH
        self.original_portfolio_path = execute_cycle.PORTFOLIO_PATH
        self.original_strategies_path = execute_cycle.STRATEGIES_PATH
        self.original_signals_path = execute_cycle.SIGNALS_PATH

        execute_cycle.HISTORY_PATH = self.history_file
        # Point to an existing file so check passes
        execute_cycle.CLI_PATH = os.path.abspath(__file__)
        execute_cycle.PORTFOLIO_PATH = "test_portfolio.md"
        # strategies.md check?
        # get_active_strategy checks STRATEGIES_PATH. If missing, defaults to BollingerBands.
        # Mock SIGNALS_PATH to avoid checking actual signals which have side matching issues
        execute_cycle.SIGNALS_PATH = "test_signals.md"
        with open(execute_cycle.SIGNALS_PATH, "w") as f:
            f.write("")
        # Mock active strategies to just one so it doesn't conflict or do nothing
        execute_cycle.STRATEGIES_PATH = "test_strategies.md"
        with open(execute_cycle.STRATEGIES_PATH, "w") as f:
            f.write("BollingerBands\n")

    def tearDown(self):
        execute_cycle.HISTORY_PATH = self.original_history_path
        execute_cycle.CLI_PATH = self.original_cli_path
        execute_cycle.PORTFOLIO_PATH = self.original_portfolio_path
        execute_cycle.STRATEGIES_PATH = self.original_strategies_path
        execute_cycle.SIGNALS_PATH = self.original_signals_path

        if os.path.exists(self.history_file):
            os.remove(self.history_file)
        if os.path.exists("test_portfolio.md"):
            os.remove("test_portfolio.md")
        if os.path.exists("test_signals.md"):
            os.remove("test_signals.md")
        if os.path.exists("test_strategies.md"):
            os.remove("test_strategies.md")

    @patch("execute_cycle.run_command")
    def test_history_update(self, mock_run):
        # Setup mock responses
        def side_effect(args):
            cmd = args[0]
            if cmd == "scan-market":
                # simulate kraken crypto and equities
                if "--provider" in args and "kraken" in args:
                    if "--top-n" in args:
                        return ["BTCUSD"]
                    return ["AAPL"]
                return ["AAPL"]
            elif cmd == "fetch-market-data":
                return {"bars": [{"close": 150.0, "timestamp_unix_ms": 1600000000000}]}
            elif cmd == "analyze-market":
                return {
                    "symbol": "AAPL", # Simplification: analyze returns AAPL for both
                    "market": "equities",
                    "regime": "Bullish",
                    "volatility": "Low",
                    "confidence": 0.8,
                    "timestamp_unix_ms": 1600000000000
                }
            elif cmd == "generate-signals":
                # Need to return correct symbol to match candidate
                # But fetch_and_generate passes temp_bars_file.
                # We can cheat and return generic intent.
                # execute_cycle iterates all strategies. We just return a signal for one to avoid duplicates.
                symbol = "AAPL"
                for arg in args:
                    if "temp_bars" in arg:
                        if "BTCUSD" in arg:
                            symbol = "BTCUSD"
                        elif "ETHUSD" in arg:
                            symbol = "ETHUSD"
                        elif "SPY" in arg:
                            symbol = "SPY"
                        else:
                            # Find anything after temp_bars_ and before .json
                            match = __import__("re").search(r"temp_bars_(.*?)\.json", arg)
                            if match:
                                symbol = match.group(1).replace("_", "/")

                return [{
                    "intent_id": "test_intent",
                    "market": "equities" if symbol in ("AAPL", "SPY") else "crypto",
                    "symbol": symbol,
                    "side": "buy",
                    "size_hint": "10",
                    "confidence": 0.9,
                    "rationale": "Test",
                    "schema_version": "v0",
                    "order_type": "market",
                    "time_in_force": "day",
                    "strategy": "BollingerBands"
                }]
            elif cmd == "execute-intent":
                return {
                    "status": "executed",
                    "provider_order_id": "123",
                    "submitted_at_unix_ms": 1600000000000
                }
            elif cmd in ("get-positions", "get-buying-power", "get-selling-power", "get-open-orders", "update-signal-history"):
                if cmd == "get-buying-power":
                    return {"amount": "1000000", "currency": "USD"}
                if cmd == "get-selling-power":
                    return {"amount": "1000000", "asset": "BTCUSD"}
                return []
            return None

        mock_run.side_effect = side_effect

        # Run main logic
        # Need to ensure SIMULATION is false for test so that scan-market behaves as mocked
        original_env = os.environ.get("SIMULATION")
        os.environ["SIMULATION"] = "false"
        try:
            execute_cycle.main()
        finally:
            if original_env is not None:
                os.environ["SIMULATION"] = original_env
            else:
                del os.environ["SIMULATION"]

        # Check history.json
        self.assertTrue(os.path.exists(self.history_file), "History file should be created")
        with open(self.history_file, "r") as f:
            history = json.load(f)

        # We expect 2 entries (BTCUSD and AAPL)
        self.assertEqual(len(history), 2)

        entry = history[0]
        self.assertIn("market_analysis", entry)
        self.assertEqual(entry["market_analysis"]["regime"], "Bullish")
        self.assertIsNone(entry["outcome"])

        # Check that analyze-market was called with --no-report
        calls = [c[0][0] for c in mock_run.call_args_list]
        analyze_calls = [args for args in calls if args[0] == "analyze-market"]
        self.assertTrue(len(analyze_calls) > 0)
        self.assertIn("--no-report", analyze_calls[0])

if __name__ == "__main__":
    unittest.main()
