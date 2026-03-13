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

def search_research(symbol, bars_list):
    """
    Reads research, news, and knowledge from local text files.
    """
    research = ""
    news = ""
    knowledge = ""

    # Simple simulated logic: check if the text mentions the symbol or if it's broad enough.
    # In a real app this would query a vector DB or an API.
    # For now, we adjust our hardcoded files behavior based on the symbol so we don't return Bitcoin news for SPY.

    try:
        with open("research.txt", "r") as f:
            full_research = f.read().strip()
            if symbol == "BTCUSD" and "Bitcoin" in full_research:
                research = full_research
            elif symbol == "ETHUSD":
                research = "Ethereum sentiment is improving following network upgrades. (Source: External Research)"
            elif symbol == "SPY":
                research = "Macroeconomic data supports a soft landing. Analysts maintain overweight positions on large-cap tech. (Source: External Research)"
    except Exception:
        pass

    try:
        with open("knowledge.txt", "r") as f:
            full_knowledge = f.read().strip()
            # Knowledge seems general ("Similar market conditions in Q4 2023...") so we can apply it
            knowledge = full_knowledge
    except Exception:
        pass

    try:
        with open("news.txt", "r") as f:
            full_news = f.read().strip()
            if symbol == "BTCUSD" and "Bitcoin" in full_news:
                news = full_news
            elif symbol == "ETHUSD":
                news = "ETH outpaces major assets amid network improvements. (Source: External News)"
            elif symbol == "SPY":
                news = "Major indices are hitting new highs, SPY breaks previous all-time highs on tech earnings beat. (Source: External News)"
    except Exception:
        pass

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

    # Note: We do NOT use --no-report because we WANT it to update Signals.md and others.
    # The command returns the analysis JSON.
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

        # Save temp bars file
        temp_bars_file = f"temp_bars_{symbol}.json"
        with open(temp_bars_file, "w") as f:
            json.dump(data, f)

        # 2. Search Research (Simulated)
        research, news = search_research(symbol, bars_list)

        # 3. Analyze Market & Generate Report
        analysis = analyze_market(symbol, temp_bars_file, research, news)

        if analysis:
            print(f"\n--- Analysis for {symbol} ---")
            print(json.dumps(analysis, indent=2))

            # Check for alerts
            regime = analysis.get("regime", "")
            if "Trending" in regime:
                 print(f"ALERT: Strong Trend Detected: {regime}")
            if analysis.get("volatility") == "High" or analysis.get("volatility") == "Extreme":
                 print(f"ALERT: High Volatility Detected!")

        # Cleanup
        if os.path.exists(temp_bars_file):
            os.remove(temp_bars_file)

if __name__ == "__main__":
    main()
