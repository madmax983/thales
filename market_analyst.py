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
    Simulates searching for research/news.
    In a real scenario, this would call external search APIs.
    Here, it generates context-aware synthetic news based on price action (simulation).
    """
    if not bars_list or len(bars_list) < 2:
        return None, None

    last_close = bars_list[-1].get("close", 0.0)
    prev_close = bars_list[-2].get("close", 0.0)
    change_pct = (last_close - prev_close) / prev_close if prev_close > 0 else 0.0

    research = ""
    news = ""

    if change_pct > 0.02:
        research = "Simulated Environment: Strong uptrend detected. Market sentiment appears extremely bullish, likely driven by simulated positive macroeconomic news or sector rotation."
        news = "Simulated News: Major indices/assets hit new highs. Positive earnings reports driving momentum."
    elif change_pct < -0.02:
        research = "Simulated Environment: Sharp correction underway. Bearish sentiment dominant. High volume selling suggests institutional liquidation."
        news = "Simulated News: Regulatory concerns resurface. Major exchange outflow detected."
    elif change_pct > 0.005:
        research = "Simulated Environment: Steady accumulation observed. Technical indicators suggest continuation of the trend."
        news = "Simulated News: Analyst upgrades for key sectors. Optimism regarding future growth."
    elif change_pct < -0.005:
        research = "Simulated Environment: Profit taking observed near resistance levels. Short-term bearish divergence."
        news = "Simulated News: Mixed economic data causes market uncertainty."
    else:
        research = "Simulated Environment: Market consolidation. Low volatility suggests a potential breakout or breakdown soon."
        news = "Simulated News: Quiet trading session ahead of major economic announcements."

    return research, news

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
