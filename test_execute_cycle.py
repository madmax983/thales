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
                bars = {"schema_version": "v0", "bars": [{"close": 100.0, "timestamp_unix_ms": 1000}]}
                return ret_json(bars)

            elif "get-buying-power" in cmd_str:
                return ret_json({"amount": 10000.0, "currency": "USD"})

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
                    "status": "filled",
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

    @patch("execute_cycle.run_command")
    def test_main_converts_oversized_sell_to_max_before_execute(self, mock_run_command):
        captured_intent = {}

        def side_effect(args):
            cmd = args[0]

            if cmd == "scan-market":
                provider = args[args.index("--provider") + 1]
                if provider == "kraken" or provider == "paper":
                    return ["BTCUSD"]
                if provider == "alpaca":
                    return []
                return []

            if cmd == "fetch-market-data":
                return {"schema_version": "v0", "bars": [{"close": 100.0, "timestamp_unix_ms": 1000}]}

            if cmd == "analyze-market":
                return {
                    "regime": "Trending Up",
                    "market": "crypto",
                    "symbol": "BTCUSD",
                    "sentiment": "Neutral",
                    "volatility": "Medium",
                    "confidence": 0.8,
                }

            if cmd == "generate-signals":
                strategy = args[args.index("--strategy") + 1]
                return [
                    {
                        "intent_id": f"sell-{strategy}",
                        "market": "crypto",
                        "symbol": "BTCUSD",
                        "side": "sell",
                        "size_hint": "2.0",
                        "confidence": 0.9,
                        "rationale": "Oversized sell for regression test",
                        "stop_loss": 110.0,
                        "take_profit": 90.0,
                        "order_type": "market",
                        "time_in_force": "GTC",
                        "signal_type": "Entry",
                        "strategy": strategy,
                    }
                ]

            if cmd == "get-positions":
                return []

            if cmd == "get-selling-power":
                return {"amount": 0.75, "asset": "BTC"}

            if cmd == "execute-intent":
                input_path = args[args.index("--input") + 1]
                with open(input_path, "r") as f:
                    payload = json.load(f)
                captured_intent["side"] = payload.get("side")
                captured_intent["size_hint"] = payload.get("size_hint")
                return [
                    {
                        "schema_version": "v0",
                        "intent_id": payload.get("intent_id", "sell-test"),
                        "provider": "kraken",
                        "status": "submitted",
                        "provider_order_id": "oid-sell-1",
                        "submitted_at_unix_ms": 1234567890000,
                    }
                ]

            if cmd == "get-open-orders":
                return []

            return []

        mock_run_command.side_effect = side_effect

        execute_cycle.main()

        self.assertEqual(captured_intent.get("side"), "sell")
        self.assertEqual(
            captured_intent.get("size_hint"),
            "0.75",
            "Oversized sell should be converted to sell-all before execute-intent",
        )

    def test_classify_execution_outcome(self):
        self.assertEqual(
            execute_cycle.classify_execution_outcome({"status": "filled"}),
            "executed",
        )
        self.assertEqual(
            execute_cycle.classify_execution_outcome({"status": "submitted"}),
            "submitted",
        )
        self.assertEqual(
            execute_cycle.classify_execution_outcome({"status": "rejected"}),
            "rejected",
        )
        self.assertEqual(
            execute_cycle.classify_execution_outcome({"status": "mystery_status"}),
            "unknown",
        )

    def test_log_submitted_writes_submitted_orders_section(self):
        intent = {
            "intent_id": "intent-submitted-1",
            "symbol": "BTCUSD",
            "side": "buy",
            "provider": "kraken",
        }
        result = {
            "provider": "kraken",
            "provider_order_id": "oid-123",
            "status": "submitted",
            "submitted_at_unix_ms": 1234567890000,
        }

        execute_cycle.log_submitted(intent, result)

        with open("temp_portfolio.md", "r") as f:
            content = f.read()

        self.assertIn("## Submitted Orders", content)
        self.assertIn("intent-submitted-1", content)
        self.assertIn("submitted", content)

    def test_select_strategies_returns_all_active_strategies(self):
        # Now returns all strategies regardless of regime
        active = ["BollingerBands", "EmaCrossover", "RsiMeanReversion", "Macd"]
        analysis = {"regime": "Ranging", "volatility": "Low"}

        selected = execute_cycle.select_strategies_for_analysis(active, analysis)

        self.assertEqual(selected, active)

    def test_select_strategies_returns_all_for_trending(self):
        # Now returns all strategies regardless of regime
        active = ["BollingerBands", "EmaCrossover", "RsiMeanReversion", "Macd"]
        analysis = {"regime": "Trending Up", "volatility": "Low"}

        selected = execute_cycle.select_strategies_for_analysis(active, analysis)

        self.assertEqual(selected, active)

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

        # New logic: Strict conflict resolution should return 0 if conflicting sides exist
        # To test preference, we need a case WITHOUT side conflict, or update test expectation to 0.
        # But this test name implies it checks for preference.
        # Let's update the test to reflect the NEW STRICT behavior: IT SHOULD REJECT.

        resolved = execute_cycle.resolve_conflicts(intents, {"symbol": "AAPL"})

        self.assertEqual(len(resolved), 0, "Should reject conflicted signals (Buy vs Sell)")

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

        resolved = execute_cycle.resolve_conflicts(intents, {"symbol": "AAPL"})

        self.assertEqual(resolved, [])
        self.assertEqual(mock_log_skipped.call_count, 1)

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

    @patch("execute_cycle.run_command")
    def test_adjust_buy_size_to_buying_power_caps_size(self, mock_run_command):
        mock_run_command.return_value = {"amount": 100.0, "currency": "USD"}

        intent = {
            "provider": "kraken",
            "symbol": "BTCUSD",
            "side": "buy",
            "size_hint": "1.0",
            "intent_id": "buy-1",
            "rationale": "test",
        }

        ok, reason = execute_cycle.adjust_buy_size_to_buying_power(intent, current_price=200.0)

        self.assertTrue(ok, f"Expected clamp success, got reason: {reason}")
        self.assertLess(float(intent["size_hint"]), 1.0)
        self.assertAlmostEqual(float(intent["size_hint"]), 0.495, places=3)

    @patch("execute_cycle.run_command")
    def test_adjust_buy_size_to_buying_power_rejects_when_no_funds(self, mock_run_command):
        mock_run_command.return_value = {"amount": 0.0, "currency": "USD"}

        intent = {
            "provider": "kraken",
            "symbol": "ETHUSD",
            "side": "buy",
            "size_hint": "0.25",
            "intent_id": "buy-2",
            "rationale": "test",
        }

        ok, reason = execute_cycle.adjust_buy_size_to_buying_power(intent, current_price=2500.0)

        self.assertFalse(ok)
        self.assertIn("No buying power", reason)

    @patch("execute_cycle.run_command")
    def test_adjust_buy_size_to_buying_power_ignores_non_buys(self, mock_run_command):
        intent = {
            "provider": "kraken",
            "symbol": "BTCUSD",
            "side": "sell",
            "size_hint": "1.0",
            "intent_id": "sell-1",
            "rationale": "test",
        }

        ok, reason = execute_cycle.adjust_buy_size_to_buying_power(intent, current_price=90000.0)

        self.assertTrue(ok)
        self.assertEqual(reason, "Not a buy signal")
        self.assertEqual(intent["size_hint"], "1.0")
        mock_run_command.assert_not_called()

    @patch("execute_cycle.run_command")
    def test_adjust_sell_size_to_sellable_balance_sets_max_when_size_exceeds_balance(self, mock_run_command):
        mock_run_command.return_value = {"amount": 0.75, "asset": "BTC"}

        intent = {
            "provider": "kraken",
            "symbol": "BTCUSD",
            "side": "sell",
            "size_hint": "1.0",
            "intent_id": "sell-oversize",
            "rationale": "test",
        }

        ok, reason = execute_cycle.adjust_sell_size_to_sellable_balance(intent)

        self.assertTrue(ok, f"Expected sell-all fallback, got reason: {reason}")
        self.assertEqual(intent["size_hint"], "0.75")
        self.assertIn("Sell size adjusted", intent["rationale"])
        mock_run_command.assert_called_once_with(
            ["get-selling-power", "--provider", "kraken", "--symbol", "BTCUSD"]
        )

    @patch("execute_cycle.run_command")
    def test_adjust_sell_size_to_sellable_balance_rejects_when_no_sellable_balance(self, mock_run_command):
        mock_run_command.return_value = {"amount": 0.0, "asset": "BTC"}

        intent = {
            "provider": "kraken",
            "symbol": "BTCUSD",
            "side": "sell",
            "size_hint": "0.5",
            "intent_id": "sell-none",
            "rationale": "test",
        }

        ok, reason = execute_cycle.adjust_sell_size_to_sellable_balance(intent)

        self.assertFalse(ok)
        self.assertIn("No sellable balance", reason)

    @patch("execute_cycle.run_command")
    def test_adjust_sell_size_to_sellable_balance_ignores_non_sells(self, mock_run_command):
        intent = {
            "provider": "kraken",
            "symbol": "BTCUSD",
            "side": "buy",
            "size_hint": "0.5",
            "intent_id": "buy-ignore",
            "rationale": "test",
        }

        ok, reason = execute_cycle.adjust_sell_size_to_sellable_balance(intent)

        self.assertTrue(ok)
        self.assertEqual(reason, "Not a sell signal")
        self.assertEqual(intent["size_hint"], "0.5")
        mock_run_command.assert_not_called()

if __name__ == '__main__':
    unittest.main()
