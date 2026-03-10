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
    Simulates searching for research/news with citations.
    """
    if not bars_list or len(bars_list) < 2:
        return None, None

    last_close = bars_list[-1].get("close", 0.0)
    prev_close = bars_list[-2].get("close", 0.0)
    change_pct = (last_close - prev_close) / prev_close if prev_close > 0 else 0.0

    research = ""
    news = ""

    if "BTC" in symbol or "ETH" in symbol:
        asset_type = "crypto"
        source_r = "Crypto Research Hub"
        source_n = "Financial Times"
        if change_pct > 0.01:
            research = f"{symbol} network activity is surging, with decentralized protocols showing increased volume (Source: {source_r})."
            news = f"{symbol} pushes past key resistance levels amidst broader {asset_type} market rally (Source: {source_n})."
        elif change_pct < -0.01:
            research = f"Simulated Environment: Sharp correction underway for {symbol}. Bearish sentiment dominant (Source: {source_r})."
            news = f"Regulatory concerns resurface for {asset_type}. Major exchange outflow detected (Source: {source_n})."
        else:
            research = f"{symbol} ETFs and institutional inflows remain steady following recent approvals. Analysts remain bullish (Source: {source_r})."
            news = f"Major indices are hitting new highs, with {symbol} showing resilience against recent regulatory concerns (Source: {source_n})."
    else:
        asset_type = "equities"
        source_r = "Goldman Sachs Equity Research"
        source_n = "Wall Street Journal"
        if change_pct > 0.01:
            research = f"S&P 500 companies are reporting robust quarterly earnings, exceeding analyst expectations in tech and energy sectors (Source: {source_r})."
            news = f"US stock market hits record highs as inflation concerns ease and corporate profits soar (Source: {source_n})."
        elif change_pct < -0.01:
            research = f"Profit taking observed across {asset_type}. Short-term bearish divergence detected (Source: {source_r})."
            news = f"Mixed economic data causes market uncertainty for {symbol} (Source: {source_n})."
        else:
            research = f"Steady accumulation observed in {asset_type}. Technical indicators suggest continuation of the trend. The market is pricing in favorable interest rate policies (Source: {source_r})."
            news = f"Analyst upgrades for key sectors. Optimism regarding future growth (Source: {source_n})."

    return research, news

def search_knowledge(symbol, bars_list):
    """
    Simulates searching for historical knowledge context with citations.
    """
    return f"Similar market conditions in Q4 2023 showed a strong trend following consolidation periods for {symbol} (Source: Thales Knowledge Base)."

def get_previous_regime(symbol):
    """Reads the last recorded regime for the symbol from Market_Regime.md to detect actual changes."""
    regime_file = "Market_Regime.md"
    if not os.path.exists(regime_file):
        return None

    last_regime = None
    with open(regime_file, "r") as f:
        lines = f.readlines()
        # Search backwards for the last regime of this symbol
        symbol_header = f"### {symbol}"
        for i in range(len(lines) - 1, -1, -1):
            if lines[i].startswith("**Regime**:"):
                # Check if the block belongs to the symbol
                for j in range(i, max(-1, i-5), -1):
                    if lines[j].startswith(symbol_header):
                        return lines[i].replace("**Regime**:", "").strip()
    return None

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

        # 2. Search Research & Knowledge (Simulated)
        research, news = search_research(symbol, bars_list)
        knowledge = search_knowledge(symbol, bars_list)

        # Combine knowledge into research for context
        combined_research = f"{research} {knowledge}" if research else knowledge

        # Determine previous regime for genuine alerts
        prev_regime = get_previous_regime(symbol)

        # 3. Analyze Market & Generate Report (CLI will natively append to .md logs)
        analysis = analyze_market(symbol, temp_bars_file, combined_research, news)

        if analysis:
            regime = analysis.get("regime", "")
            sentiment = analysis.get("sentiment", "")
            confidence = analysis.get("confidence", 0.0)
            volatility = analysis.get("volatility", "")
            patterns = analysis.get("patterns", [])
            key_levels = analysis.get("key_levels", [])
            assessment = analysis.get("recommendation", "")

            print(f"\n--- Analysis for {symbol} ---")
            print(f"Regime: {regime}")

            # Check for genuine alerts
            if prev_regime and prev_regime != regime:
                 print(f"ALERT: Significant Regime Change Detected! (Previous: {prev_regime}, Current: {regime})")
            elif not prev_regime and "Trending" in regime:
                 print(f"ALERT: Strong Trend Detected: {regime}")

            print(f"Sentiment: {sentiment}")
            print(f"Patterns: {', '.join(patterns) if patterns else 'None'}")
            print(f"Key Levels: {', '.join(map(str, key_levels))}")
            print(f"Volatility: {volatility}")

            if volatility in ["High", "Extreme"]:
                 print(f"ALERT: Unusual Volatility Detected! Level: {volatility}")

            print(f"Confidence: {confidence * 100:.2f}%")
            print(f"Strategy Adjustments: {assessment}")

        # Cleanup
        if os.path.exists(temp_bars_file):
            os.remove(temp_bars_file)

if __name__ == "__main__":
    main()
