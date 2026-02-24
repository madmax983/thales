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
        execute_cycle.HISTORY_PATH = self.history_file
        # Point to an existing file so check passes
        execute_cycle.CLI_PATH = os.path.abspath(__file__)
        execute_cycle.PORTFOLIO_PATH = "test_portfolio.md"
        # strategies.md check?
        # get_active_strategy checks STRATEGIES_PATH. If missing, defaults to BollingerBands.
        execute_cycle.STRATEGIES_PATH = "strategies.md"

    def tearDown(self):
        if os.path.exists(self.history_file):
            os.remove(self.history_file)
        if os.path.exists("test_portfolio.md"):
            os.remove("test_portfolio.md")

    @patch("execute_cycle.run_command")
    def test_history_update(self, mock_run):
        # Setup mock responses
        def side_effect(args):
            cmd = args[0]
            if cmd == "scan-market":
                if "--provider" in args and "kraken" in args:
                    return ["BTCUSD"]
                return ["AAPL"]
            elif cmd == "fetch-market-data":
                return [{"close": 150.0, "timestamp_unix_ms": 1600000000000}]
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
                return [{
                    "intent_id": "test_intent",
                    "market": "equities",
                    "symbol": "AAPL",
                    "side": "buy",
                    "size_hint": "10",
                    "confidence": 0.9,
                    "rationale": "Test",
                    "schema_version": "v0",
                    "order_type": "market",
                    "time_in_force": "day"
                }]
            elif cmd == "execute-intent":
                return {
                    "status": "submitted",
                    "provider_order_id": "123",
                    "submitted_at_unix_ms": 1600000000000
                }
            return None

        mock_run.side_effect = side_effect

        # Run main logic
        with patch("builtins.print"):
            execute_cycle.main()

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
