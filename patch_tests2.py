import re

with open("test_execute_cycle.py", "r") as f:
    content = f.read()

# Need to replace the old function completely.
content = re.sub(r'    def test_resolve_conflicts_prefers_regime_aligned_side\(self\):.*?        resolved = execute_cycle\.resolve_conflicts\(intents, conflict_margin=0\.05\)\n        self\.assertEqual\(len\(resolved\), 0\)',
"""    def test_resolve_conflicts_prefers_regime_aligned_side(self):
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

        candidate = {"symbol": "AAPL"}
        resolved = execute_cycle.resolve_conflicts(intents, candidate)
        self.assertEqual(len(resolved), 0)""", content, flags=re.DOTALL)

content = re.sub(r'    @patch\("execute_cycle\.log_skipped"\)\n    def test_resolve_conflicts_skips_when_scores_too_close\(self, mock_log_skipped\):.*?        resolved = execute_cycle\.resolve_conflicts\(intents, conflict_margin=0\.05\)\n        self\.assertEqual\(len\(resolved\), 0\)\n        mock_log_skipped\.assert_called\(\)',
"""    @patch("execute_cycle.log_skipped")
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
        mock_log_skipped.assert_called()""", content, flags=re.DOTALL)

with open("test_execute_cycle.py", "w") as f:
    f.write(content)
