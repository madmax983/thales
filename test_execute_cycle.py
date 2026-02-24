import unittest
from unittest.mock import patch, MagicMock
import json
import os
import shutil
import execute_cycle

class TestExecuteCycle(unittest.TestCase):
    def setUp(self):
        # Backup portfolio.md
        if os.path.exists("portfolio.md"):
            shutil.copy("portfolio.md", "portfolio.md.bak")
        # Ensure cleanup if previous test failed
        if os.path.exists("temp_portfolio.md"):
            os.remove("temp_portfolio.md")

        # Override PORTFOLIO_PATH in execute_cycle for test isolation
        self.original_portfolio_path = execute_cycle.PORTFOLIO_PATH
        execute_cycle.PORTFOLIO_PATH = "temp_portfolio.md"

        # Override HISTORY_PATH in execute_cycle for test isolation
        self.original_history_path = execute_cycle.HISTORY_PATH
        execute_cycle.HISTORY_PATH = "temp_history.json"
        if os.path.exists("temp_history.json"):
            os.remove("temp_history.json")

    def tearDown(self):
        # Restore PORTFOLIO_PATH
        execute_cycle.PORTFOLIO_PATH = self.original_portfolio_path
        execute_cycle.HISTORY_PATH = self.original_history_path

        # Remove temp portfolio
        if os.path.exists("temp_portfolio.md"):
            os.remove("temp_portfolio.md")

        # Remove temp history
        if os.path.exists("temp_history.json"):
            os.remove("temp_history.json")

        # Restore original portfolio if backup exists (though we didn't modify it)
        if os.path.exists("portfolio.md.bak"):
            # If original exists, we didn't touch it, but just in case
            # os.remove("portfolio.md")
            # shutil.move("portfolio.md.bak", "portfolio.md")
            os.remove("portfolio.md.bak")

    @patch('execute_cycle.subprocess.run')
    def test_full_cycle_execution(self, mock_run):
        # Setup mock behavior
        def side_effect(cmd, **kwargs):
            mock_ret = MagicMock()
            mock_ret.returncode = 0

            # Helper to return JSON
            def ret_json(data):
                mock_ret.stdout = json.dumps({"status": "ok", "data": data})
                return mock_ret

            cmd_str = " ".join(cmd)

            if "scan-market" in cmd_str:
                return ret_json(["MOCKUSD"])

            elif "fetch-market-data" in cmd_str:
                # Return minimal bars
                bars = [{"close": 100.0, "timestamp_unix_ms": 1000}]
                return ret_json(bars)

            elif "analyze-market" in cmd_str:
                return ret_json({"regime": "Bullish"})

            elif "generate-signals" in cmd_str:
                # Return a valid mock signal
                signal = [{
                    "intent_id": "test_id",
                    "market": "crypto",
                    "symbol": "MOCKUSD",
                    "side": "buy",
                    "size_hint": "0.1",
                    "confidence": 0.9,
                    "rationale": "Test Signal",
                    "stop_loss": 90.0, # Required for risk check
                    "limit_price": 100.0,
                    "order_type": "limit",
                    "time_in_force": "GTC",
                    "signal_type": "Entry" # Helps with risk check logic
                }]
                return ret_json(signal)

            elif "execute-intent" in cmd_str:
                result = [{
                    "schema_version": "v0",
                    "intent_id": "test_id",
                    "provider": "kraken",
                    "status": "submitted",
                    "provider_order_id": "123",
                    "submitted_at_unix_ms": 1234567890000
                }]
                return ret_json(result)

            elif "get-positions" in cmd_str:
                # Return mock position to test ScaleIn logic if needed
                # For now just return empty list or a dummy one
                return ret_json([])

            # Default empty
            return ret_json([])

        mock_run.side_effect = side_effect

        # Run main
        execute_cycle.main()

        # Check if portfolio was written
        self.assertTrue(os.path.exists("temp_portfolio.md"), "Portfolio file should be created")

        with open("temp_portfolio.md", "r") as f:
            content = f.read()

        print(f"Portfolio Content:\n{content}")

        self.assertIn("MOCKUSD", content)
        self.assertIn("buy", content)
        self.assertIn("0.1", content)
        self.assertIn("Test Signal", content)

    @patch('execute_cycle.subprocess.run')
    def test_verify_risk_rejects_sell_without_position(self, mock_run):
         # Setup mock behavior
        def side_effect(cmd, **kwargs):
            mock_ret = MagicMock()
            mock_ret.returncode = 0
            def ret_json(data):
                mock_ret.stdout = json.dumps({"status": "ok", "data": data})
                return mock_ret

            cmd_str = " ".join(cmd)
            if "scan-market" in cmd_str:
                return ret_json(["MOCKUSD"])
            elif "fetch-market-data" in cmd_str:
                 return ret_json([{"close": 100.0, "timestamp_unix_ms": 1000}])
            elif "analyze-market" in cmd_str:
                return ret_json({"regime": "Bearish"})
            elif "get-positions" in cmd_str:
                return ret_json([]) # NO POSITIONS
            elif "generate-signals" in cmd_str:
                # Generate a SELL signal
                signal = [{
                    "intent_id": "test_sell_id",
                    "market": "crypto",
                    "symbol": "MOCKUSD",
                    "side": "sell", # SELL!
                    "size_hint": "0.1",
                    "confidence": 0.9,
                    "rationale": "Test Sell Signal",
                    "limit_price": 100.0,
                    "order_type": "limit",
                    "time_in_force": "GTC",
                    "signal_type": "Exit"
                }]
                return ret_json(signal)

            return ret_json([])

        mock_run.side_effect = side_effect

        execute_cycle.main()

        with open("temp_portfolio.md", "r") as f:
            content = f.read()

        print(f"Portfolio Content (Sell Test):\n{content}")
        # Should contain rejection log
        self.assertIn("Rejected by Risk Agent", content)
        self.assertIn("Sell signal received but no open position", content)
        # Should NOT contain "submitted" execution log (which has | buy | or | sell | format)
        # The rejection log has | MOCKUSD | test_sell_id | Rejected... |
        # The execution log has | ... | MOCKUSD | sell | ... |

        self.assertIn("| test_sell_id |", content)

if __name__ == '__main__':
    unittest.main()
