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

    try:
        with open("research.txt", "r") as f:
            research = f.read().strip()
    except Exception:
        pass

    try:
        with open("knowledge.txt", "r") as f:
            knowledge = f.read().strip()
    except Exception:
        pass

    try:
        with open("news.txt", "r") as f:
            news = f.read().strip()
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

            # Determine formatted date string
            from datetime import datetime
            dt = datetime.fromtimestamp(analysis.get("timestamp_unix_ms", 0) / 1000.0)
            formatted_date = dt.strftime("%Y-%m-%d %H:%M:%S")

            market = analysis.get("market", "")
            sentiment = analysis.get("sentiment", "")
            confidence = analysis.get("confidence", 0.0)
            volatility = analysis.get("volatility", "")
            atr = analysis.get("atr")
            if atr is not None:
                atr_str = f"{atr:.2f}"
            else:
                atr_str = "N/A"
            assessment = analysis.get("recommendation", "")
            research_summary = analysis.get("research_summary", "None")
            news_summary = analysis.get("news_summary", "None")

            # Update Market_Regime.md
            with open("Market_Regime.md", "a") as f:
                f.write(f"\n### {symbol} - {formatted_date} ({market})\n")
                f.write(f"**Regime**: {regime}\n")
                f.write(f"**Sentiment**: {sentiment}\n")
                f.write(f"**Confidence**: {confidence * 100:.2f}%\n")

            # Update Volatility_Regime.md
            with open("Volatility_Regime.md", "a") as f:
                f.write(f"\n### {symbol} - {formatted_date} ({market})\n")
                f.write(f"**Volatility**: {volatility}\n")
                f.write(f"**ATR**: {atr_str}\n")
                f.write(f"**Assessment**: {assessment}\n")

            # Update Market_Research.md
            with open("Market_Research.md", "a") as f:
                f.write(f"\n### {symbol} - {formatted_date} ({market})\n")
                f.write(f"**Research**: {research_summary}\n")
                f.write(f"**News**: {news_summary}\n")

        # Cleanup
        if os.path.exists(temp_bars_file):
            os.remove(temp_bars_file)

if __name__ == "__main__":
    main()
