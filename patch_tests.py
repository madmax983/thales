import re

with open("test_execute_cycle.py", "r") as f:
    content = f.read()

new_test1 = """
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
                "intent_id": "sell-bb",
                "symbol": "AAPL",
                "side": "sell",
                "confidence": 0.75,
                "strategy_used": "BollingerBands",
                "_market_analysis": analysis,
            },
        ]

        candidate = {"symbol": "AAPL"}
        resolved = execute_cycle.resolve_conflicts(intents, candidate)
        self.assertEqual(len(resolved), 0)
"""

content = re.sub(
    r'    def test_resolve_conflicts_prefers_regime_aligned_side.*?resolved = execute_cycle\.resolve_conflicts\(intents, conflict_margin=0\.05\)\s*self\.assertEqual\(len\(resolved\), 0\)',
    new_test1.strip(),
    content,
    flags=re.DOTALL
)


new_test2 = """
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

        candidate = {"symbol": "ETHUSD"}
        resolved = execute_cycle.resolve_conflicts(intents, candidate)
        self.assertEqual(len(resolved), 0)
        mock_log_skipped.assert_called()
"""

content = re.sub(
    r'    @patch\("execute_cycle\.log_skipped"\)\n    def test_resolve_conflicts_skips_when_scores_too_close.*?mock_log_skipped\.assert_called\(\)',
    new_test2.strip(),
    content,
    flags=re.DOTALL
)

with open("test_execute_cycle.py", "w") as f:
    f.write(content)
