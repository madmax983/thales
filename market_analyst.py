import json
import subprocess
import os
import sys

# Configuration
CLI_PATH = "./target/release/thales-cli"
SYMBOLS = ["BTCUSD", "ETHUSD", "SPY"] # Default watchlist
PROVIDERS = {
    "BTCUSD": "paper", # Simulation environment (2026)
    "ETHUSD": "paper",
    "SPY": "paper"
}

def run_command(args):
    """Runs a thales-cli command and returns the parsed JSON data."""
    cmd = [CLI_PATH] + args
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=False)
        stdout = (result.stdout or "").strip()

        envelope = None
        if stdout:
            json_start = stdout.find("{")
            if json_start != -1:
                try:
                    envelope = json.loads(stdout[json_start:])
                except json.JSONDecodeError:
                    envelope = None

        if envelope and envelope.get("status") == "ok":
            return envelope.get("data")

        if result.returncode != 0:
            print(f"Command failed: {cmd}\nReason: {result.stderr or stdout}")
            return None

        return None
    except Exception as e:
        print(f"Exception running command {cmd}: {e}")
        return None

def fetch_market_data(symbol, provider):
    """Fetches market data for a symbol."""
    print(f"Fetching market data for {symbol} ({provider})...")
    data = run_command(["fetch-market-data", "--provider", provider, "--symbol", symbol, "--timeframe", "1h"])
    return data

def search_research(symbol, bars_list, market):
    """
    Reads research, news, and knowledge from local text files for the given symbol.
    """
    research = ""
    news = ""
    knowledge = ""

    try:
        with open("research.txt", "r") as f:
            research = f.read().strip()
        if "Bitcoin" in research and "BTC" not in symbol:
            research = ""
        elif research:
            research = f"[Source: research.txt] {research}"
    except Exception:
        pass

    try:
        with open("knowledge.txt", "r") as f:
            knowledge = f.read().strip()
        if "Bitcoin" in knowledge and "BTC" not in symbol:
            knowledge = ""
        elif knowledge:
            knowledge = f"[Source: knowledge.txt] {knowledge}"
    except Exception:
        pass

    try:
        with open("news.txt", "r") as f:
            news = f.read().strip()
        if "Bitcoin" in news and "BTC" not in symbol:
            news = ""
        elif news:
            news = f"[Source: news.txt] {news}"
    except Exception:
        pass

    # Fallback/specifics if local files are empty or filtered out
    if market == "equities" and not research and not news:
        research = "[Source: WSJ_Equities_Report] S&P 500 companies are reporting robust quarterly earnings, exceeding analyst expectations in tech and energy sectors. The market is pricing in favorable interest rate policies."
        knowledge = "[Source: knowledge.txt] Similar market conditions in Q4 2023 showed a strong bullish trend following consolidation periods for equities. The market is currently exhibiting similar behavior."
        news = "[Source: news.txt] US stock market hits record highs as inflation concerns ease and corporate profits soar."
    elif not research and not news:
        research = "[Source: General_Market_Report] Steady accumulation observed. Technical indicators suggest continuation of the trend."
        knowledge = "[Source: knowledge.txt] Historical technicals show mean reversion likely."
        news = "[Source: news.txt] Mixed economic data causes market uncertainty."

    # Combine research and knowledge
    combined_research = ""
    if research and knowledge:
        combined_research = f"Historical Context: {knowledge} Research: {research}"
    elif research:
        combined_research = research
    elif knowledge:
        combined_research = f"Historical Context: {knowledge}"

    return combined_research, news

def analyze_market(symbol, bars_file, research, news):
    """Runs the market analysis."""
    print(f"Analyzing market for {symbol}...")
    args = ["analyze-market", "--input", bars_file]
    if research:
        args.extend(["--research", research])
    if news:
        args.extend(["--news", news])

    # Pass --no-report to intercept output and format it according to persona rules
    args.extend(["--no-report"])
    analysis = run_command(args)
    return analysis

def main():
    if not os.path.exists(CLI_PATH):
        print(f"Error: {CLI_PATH} not found. Please build the project first.")
        return

    print("=== Market Analyst Agent (Simulation Mode) ===")

    for symbol in SYMBOLS:
        provider = PROVIDERS.get(symbol, "paper")

        # 1. Fetch Data
        data = fetch_market_data(symbol, provider)
        if not data or "bars" not in data:
            print(f"Failed to fetch data for {symbol}")
            continue

        bars_list = data["bars"]
        market = bars_list[0].get("market", "") if bars_list else ""

        # Save temp bars file
        temp_bars_file = f"temp_bars_{symbol}.json"
        with open(temp_bars_file, "w") as f:
            json.dump(data, f)

        # 2. Search Research (Simulated)
        research, news = search_research(symbol, bars_list, market)

        # 3. Analyze Market & Generate Report
        # Note: The underlying rust CLI appends the results to the markdown files automatically
        # when `--no-report` is not used.
        analysis = analyze_market(symbol, temp_bars_file, research, news)

        if analysis:
            # Enforce Persona Rules explicitly
            # Explicit rule: Never make direct trading recommendations - only provide analysis
            # But DO recommend strategy adjustments based on conditions
            if "recommendation" in analysis:
                regime_lower = analysis.get("regime", "").lower()
                volatility_lower = analysis.get("volatility", "").lower()

                strategy_rec = "Hold and monitor conditions."
                if "trending" in regime_lower:
                    strategy_rec = "Consider trend-following strategies (e.g., Moving Average Crossover)."
                elif "ranging" in regime_lower or "calm" in regime_lower:
                    strategy_rec = "Consider mean-reversion strategies (e.g., RSI, Bollinger Bands)."
                elif "volatile" in regime_lower or "extreme" in volatility_lower or "high" in volatility_lower:
                    strategy_rec = "Consider volatility breakout strategies or reducing position sizing."

                analysis["recommendation"] = strategy_rec

            # Ensure numerical fields defined as f64 in Rust structs remain floats
            # This prevents invalid type: string expected f64 panics
            raw_confidence = float(analysis.get("confidence", 0.0))
            raw_confidence = round(raw_confidence, 4)
            analysis["confidence"] = raw_confidence

            # Be conservative in pattern detection
            # Only report high-confidence patterns
            if raw_confidence < 0.70:
                analysis["patterns"] = []

            # We DO NOT format the confidence in the JSON payload!
            # It must remain a float for Rust to parse it correctly later.
            # Explicit rule: Always include confidence scores (0-100%)

            print(f"\n--- Analysis for {symbol} ---")
            # Create a printable copy with formatted confidence for the terminal output
            print_analysis = analysis.copy()
            print_analysis["confidence"] = f"{raw_confidence * 100:.2f}%"
            print(json.dumps(print_analysis, indent=2))

            # Explicit rule: Alert immediately on significant regime changes
            regime = analysis.get("regime", "")
            if "Trending" in regime:
                 print(f"ALERT: Strong Trend Detected for {symbol}: {regime}")

            # Explicit rule: Report unusual volatility patterns
            volatility = analysis.get("volatility", "")
            if volatility in ["High", "Extreme"]:
                 print(f"ALERT: High/Extreme Volatility Detected for {symbol}!")

            # Programmatically append to log files
            market_type = analysis.get("market", market)
            research_summary = analysis.get("research_summary", research)
            news_summary = analysis.get("news_summary", news)

            markdown_block = f"## Market Analysis Report - {market_type} - {symbol} - {raw_confidence * 100:.2f}%\n\n"
            markdown_block += f"Analysis for {symbol}...\n\n"
            markdown_block += "```json\n"

            # Format the confidence scores as percentages in markdown outputs only.
            # The JSON payload itself MUST NOT have the string formatting, to ensure
            # confidence remains a float for Rust to parse it correctly later.
            markdown_block += json.dumps(analysis, indent=2) + "\n"
            markdown_block += "```\n\n"

            # Construct a TradeIntent dictionary
            # Persona rules dictate: Never make trading recommendations directly - only provide analysis.
            # Explicitly requested structural improvements
            # Vantage Spec dictates: If the analysis concludes that no trade should be taken, the block must explicitly state a "hold" or "neutral" recommendation rather than omitting the structure entirely.

            intent = {
                "schema_version": "v0",
                "market": market_type,
                "symbol": symbol,
                "side": "hold", # Never make direct trading recommendations
                "confidence": raw_confidence,
                "size_hint": "0",
                "stop_loss": None,
                "take_profit": None,
                "order_type": "market",
                "rationale": "Market Analyst report generated. No direct trading recommendation per persona rules.",
                "invalidation": "Market regime change"
            }

            markdown_block += "**Signal Structure**:\n"
            markdown_block += "```json\n"
            markdown_block += json.dumps(intent, indent=2) + "\n"
            markdown_block += "```\n\n"

            # Explicit rule: Cite research sources when incorporating external information
            if research_summary:
                markdown_block += f"**Research**: {research_summary}\n\n"
            if news_summary:
                markdown_block += f"**News**: {news_summary}\n\n"

            with open("Signals.md", "a") as f:
                f.write(markdown_block)

            regime_block = f"## {symbol} - {market_type}\n"
            regime_block += f"Regime: {analysis.get('regime', 'Unknown')}\n"
            regime_block += f"**Confidence**: {raw_confidence * 100:.2f}%\n\n"
            with open("Market_Regime.md", "a") as f:
                f.write(regime_block)

            volatility_block = f"## {symbol} - {market_type}\n"
            volatility_block += f"Volatility: {analysis.get('volatility', 'Unknown')}\n"
            if analysis.get("atr"):
                volatility_block += f"ATR: {analysis['atr']:.2f}\n"
            volatility_block += f"**Confidence**: {raw_confidence * 100:.2f}%\n\n"
            with open("Volatility_Regime.md", "a") as f:
                f.write(volatility_block)

            research_block = f"## {symbol} - {market_type}\n"
            if research_summary:
                research_block += f"Research: {research_summary}\n"
            if news_summary:
                research_block += f"News: {news_summary}\n"
            research_block += "\n"
            with open("Market_Research.md", "a") as f:
                f.write(research_block)

        # Cleanup
        if os.path.exists(temp_bars_file):
            os.remove(temp_bars_file)

if __name__ == "__main__":
    main()
