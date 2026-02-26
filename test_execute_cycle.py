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

        # Override SIGNALS_PATH in execute_cycle for test isolation
        self.original_signals_path = execute_cycle.SIGNALS_PATH
        execute_cycle.SIGNALS_PATH = "temp_signals.md"
        if os.path.exists("temp_signals.md"):
            os.remove("temp_signals.md")
        # Create empty signals file
        with open("temp_signals.md", "w") as f:
            f.write("")

        # Override CLI_PATH
        self.original_cli_path = execute_cycle.CLI_PATH
        execute_cycle.CLI_PATH = "temp_cli"
        with open("temp_cli", "w") as f:
            f.write("dummy")

    def tearDown(self):
        # Restore CLI_PATH
        execute_cycle.CLI_PATH = self.original_cli_path
        if os.path.exists("temp_cli"):
            os.remove("temp_cli")

        # Restore PORTFOLIO_PATH
        execute_cycle.PORTFOLIO_PATH = self.original_portfolio_path
        execute_cycle.HISTORY_PATH = self.original_history_path
        execute_cycle.SIGNALS_PATH = self.original_signals_path

        # Remove temp portfolio
        if os.path.exists("temp_portfolio.md"):
            os.remove("temp_portfolio.md")

        # Remove temp history
        if os.path.exists("temp_history.json"):
            os.remove("temp_history.json")

        # Remove temp signals
        if os.path.exists("temp_signals.md"):
            os.remove("temp_signals.md")

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
                return ret_json({"regime": "Trending Up"})

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
                    "take_profit": 110.0, # Required for risk check
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
        self.assertIn("buy (Entry)", content)
        self.assertIn("0.1", content)
        self.assertIn("Test Signal", content)

    def test_select_strategies_for_ranging_regime(self):
        active = ["BollingerBands", "EmaCrossover", "RsiMeanReversion", "Macd"]
        analysis = {"regime": "Ranging", "volatility": "Low"}

        selected = execute_cycle.select_strategies_for_analysis(active, analysis)

        self.assertEqual(selected, ["BollingerBands", "RsiMeanReversion"])

    def test_select_strategies_for_trending_regime(self):
        active = ["BollingerBands", "EmaCrossover", "RsiMeanReversion", "Macd"]
        analysis = {"regime": "Trending Up", "volatility": "Low"}

        selected = execute_cycle.select_strategies_for_analysis(active, analysis)

        self.assertEqual(selected, ["EmaCrossover", "Macd"])

    def test_resolve_conflicts_prefers_regime_aligned_side(self):
        analysis = {"regime": "Trending Up", "volatility": "Low"}
        intents = [
            {
                "intent_id": "buy-ema",
                "symbol": "AAPL",
                "side": "buy",
                "confidence": 0.60,
                "strategy_used": "EmaCrossover",
                "_market_analysis": analysis,
            },
            {
                "intent_id": "buy-macd",
                "symbol": "AAPL",
                "side": "buy",
                "confidence": 0.55,
                "strategy_used": "Macd",
                "_market_analysis": analysis,
            },
            {
                "intent_id": "sell-bb",
                "symbol": "AAPL",
                "side": "sell",
                "confidence": 0.75,
                "strategy_used": "BollingerBands",
                "_market_analysis": analysis,
            },
        ]

        resolved = execute_cycle.resolve_conflicts(intents, conflict_margin=0.05)

        self.assertEqual(len(resolved), 1)
        self.assertEqual(resolved[0]["side"], "buy")
        self.assertEqual(resolved[0]["strategy_used"], "EmaCrossover")

    @patch("execute_cycle.log_skipped")
    def test_resolve_conflicts_skips_when_scores_too_close(self, mock_log_skipped):
        analysis = {"regime": "Unknown", "volatility": "Low"}
        intents = [
            {
                "intent_id": "buy-1",
                "symbol": "ETHUSD",
                "side": "buy",
                "confidence": 0.60,
                "strategy_used": "BollingerBands",
                "_market_analysis": analysis,
            },
            {
                "intent_id": "sell-1",
                "symbol": "ETHUSD",
                "side": "sell",
                "confidence": 0.57,
                "strategy_used": "EmaCrossover",
                "_market_analysis": analysis,
            },
        ]

        resolved = execute_cycle.resolve_conflicts(intents, conflict_margin=0.05)

        self.assertEqual(resolved, [])
        self.assertEqual(mock_log_skipped.call_count, 2)

    def test_log_trade_escapes_pipes(self):
        intent = {
            "intent_id": "test|id",
            "market": "crypto",
            "symbol": "BTC|USD",
            "side": "buy",
            "size_hint": "0.1",
            "confidence": 0.9,
            "rationale": "Test | Rationale",
            "stop_loss": 90.0,
            "take_profit": 110.0,
            "limit_price": 100.0,
            "order_type": "limit",
            "time_in_force": "GTC",
            "signal_type": "Entry"
        }
        result = {"submitted_at_unix_ms": 1000}

        execute_cycle.log_trade(intent, result)

        with open("temp_portfolio.md", "r") as f:
            content = f.read()

        # Check for escaped pipes
        self.assertIn("BTC\\|USD", content)
        self.assertIn("Test \\| Rationale", content)
        self.assertIn("test\\|id", content)
        # Ensure row structure is preserved (count pipes)
        # Expected row format: | ... | ... | ... | ... | ... | ... | ... | ... | ... | ... | ... | ... |
        # 12 columns means 13 pipes + extra escaped ones.
        lines = content.strip().split('\n')
        last_line = lines[-1]
        self.assertTrue(last_line.startswith("|"))
        self.assertTrue(last_line.endswith("|"))

        # Count unescaped pipes
        # We can replace \| with something else to count strict separators
        cleaned_line = last_line.replace("\\|", "PIPE")
        pipe_count = cleaned_line.count("|")
        self.assertEqual(pipe_count, 12, f"Expected 12 pipe separators for 11 columns, got {pipe_count}. Line: {last_line}")

if __name__ == '__main__':
    unittest.main()
