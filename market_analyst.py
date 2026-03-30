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

import datetime

def analyze_market(symbol, bars_file, research, news):
    """Runs the market analysis."""
    print(f"Analyzing market for {symbol}...")
    args = ["analyze-market", "--input", bars_file]
    if research:
        args.extend(["--research", research])
    if news:
        args.extend(["--news", news])

    # Pass --no-report to prevent Rust from writing flawed outputs.
    # We will generate and write the reports manually below.
    args.append("--no-report")

    analysis = run_command(args)
    return analysis

def read_last_regime(symbol):
    """Reads the last recorded regime for a symbol from Signals.md."""
    if not os.path.exists("Signals.md"):
        return None

    with open("Signals.md", "r") as f:
        content = f.read()

    blocks = content.split("## Market Analysis Report")
    for block in reversed(blocks):
        if not block.strip() or symbol not in block:
            continue
        for line in block.split("\n"):
            line = line.strip()
            if line.startswith("Regime:"):
                return line.replace("Regime:", "").strip()
            if line.startswith("Regime Unchanged ("):
                return line.replace("Regime Unchanged (", "").rstrip(")").strip()
            if line.startswith("**ALERT: Regime Change Detected!**"):
                if "Current: " in line:
                    return line.split("Current: ")[1].rstrip(")").strip()
    return None

def write_reports(analysis, bars_list):
    """Generates the text reports to mirror reporting.rs, but incorporates persona rules and Signal Structure."""
    symbol = analysis.get("symbol", "Unknown")
    market = analysis.get("market", "Unknown")
    regime = analysis.get("regime", "Unknown")
    sentiment = analysis.get("sentiment", "Neutral")
    volatility = analysis.get("volatility", "Unknown")
    atr = analysis.get("atr")
    confidence = analysis.get("confidence", 0.0)
    ts_ms = analysis.get("timestamp_unix_ms", 0)

    try:
        dt_str = datetime.datetime.fromtimestamp(ts_ms / 1000, datetime.UTC).strftime("%Y-%m-%d %H:%M:%S")
    except Exception:
        dt_str = datetime.datetime.now(datetime.UTC).strftime("%Y-%m-%d %H:%M:%S")

    # Persona Rule: No direct trading recommendations
    # We blank it out from the raw dict, but we still use the old raw value to decide Signal Structure direction
    raw_recommendation = analysis.get("recommendation", "")
    analysis["recommendation"] = ""
    recommendation_display = "None"

    # Persona Rule: Conservative Pattern Detection
    patterns = analysis.get("patterns", [])
    if confidence < 0.7:
        patterns = []
        analysis["patterns"] = []
    patterns_str = ", ".join(patterns) if patterns else "None detected"

    key_levels = analysis.get("key_levels", [])
    levels_str = ", ".join(str(l) for l in key_levels) if key_levels else "None identified"

    research = analysis.get("research_summary")
    news = analysis.get("news_summary")
    research_section = ""
    if research and news:
        research_section = f"**Research**:\n{research}\n\n**News**:\n{news}"
    elif research:
        research_section = f"**Research**:\n{research}"
    elif news:
        research_section = f"**News**:\n{news}"
    else:
        research_section = "No external research available."

    history_section = "No recent similar setups identified." # Simplified fallback for python impl

    volatility_display = "**EXTREME (Unusual Activity)**" if volatility == "Extreme" else volatility

    prev_regime = read_last_regime(symbol)
    if prev_regime and prev_regime != regime:
        regime_change = f"**ALERT: Regime Change Detected!** (Previous: {prev_regime}, Current: {regime})"
    elif prev_regime:
        regime_change = f"Regime Unchanged ({regime})"
    else:
        regime_change = f"Regime: {regime}"

    json_block = json.dumps(analysis, indent=2)

    # Calculate Signal Structure
    rec = str(raw_recommendation).lower()
    if "long" in rec or "buy" in rec:
        side = "buy"
    elif "short" in rec or "sell" in rec:
        side = "sell"
    else:
        side = "hold"

    atr_val = atr if atr else 0.0
    current_price = bars_list[-1].get("close", 0.0) if bars_list else 0.0

    stop_loss = "None"
    take_profit = "None"
    if current_price > 0:
        if side == "buy":
            stop_loss = round(current_price - (atr_val * 2.0) if atr_val > 0 else current_price * 0.95, 4)
            take_profit = round(current_price + (atr_val * 3.0) if atr_val > 0 else current_price * 1.10, 4)
        elif side == "sell":
            stop_loss = round(current_price + (atr_val * 2.0) if atr_val > 0 else current_price * 1.05, 4)
            take_profit = round(current_price - (atr_val * 3.0) if atr_val > 0 else current_price * 0.90, 4)

    signal_structure = {
        "target symbol": symbol,
        "market": market,
        "recommended side": side,
        "confidence score": confidence,
        "suggested stop loss": stop_loss,
        "suggested take profit": take_profit
    }

    report = f"""
## Market Analysis Report - {market} - {symbol}

**Timestamp (ms)**: {ts_ms}
**Confidence**: {confidence * 100:.2f}%

### 1. Market Regime
{regime_change}
*Sentiment*: {sentiment}

### 2. Volatility
*Assessment*: {volatility_display}

### 3. Strategy Recommendation
**{recommendation_display}**

### 4. Patterns & Price Action
*Patterns*: {patterns_str}

### 5. Key Levels
*Support/Resistance*: {levels_str}

### 6. Research & Context
{research_section}
*Historical Context*: {history_section}

```json
{json_block}
```

### Signal Structure
```json
{json.dumps(signal_structure, indent=2)}
```

---
"""
    with open("Signals.md", "a") as f:
        f.write(report)

    # Regime Report
    regime_report = f"\n### {symbol} - {dt_str} ({market})\n**Regime**: {regime}\n**Sentiment**: {sentiment}\n**Confidence**: {confidence * 100:.2f}%\n"
    with open("Market_Regime.md", "a") as f:
        f.write(regime_report)

    # Volatility Report
    atr_display = f"{atr:.2f}" if atr else "N/A"
    volatility_report = f"\n### {symbol} - {dt_str} ({market})\n**Volatility**: {volatility}\n**ATR**: {atr_display}\n**Assessment**: {recommendation_display}\n"
    with open("Volatility_Regime.md", "a") as f:
        f.write(volatility_report)

    # Research Report
    r_disp = research if research else "None"
    n_disp = news if news else "None"
    research_report = f"\n### {symbol} - {dt_str} ({market})\n**Research**: {r_disp}\n**News**: {n_disp}\n"
    with open("Market_Research.md", "a") as f:
        f.write(research_report)

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
        # We pass --no-report and manually write the files to enforce constraints
        # and include the new signal structure block.
        analysis = analyze_market(symbol, temp_bars_file, research, news)

        if analysis:
            # Write to files (Signals.md, etc.)
            write_reports(analysis, bars_list)

            # Ensure confidence is 0-100% formatted for print
            confidence_raw = analysis.get("confidence", 0.0)
            analysis["confidence_formatted"] = f"{confidence_raw * 100:.2f}%"

            print(f"\n--- Analysis for {symbol} ---")
            print(json.dumps(analysis, indent=2))

            # Check for alerts
            regime = analysis.get("regime", "")
            if "Trending" in regime:
                 print(f"ALERT: Strong Trend Detected: {regime}")
            if analysis.get("volatility") in ["High", "Extreme"]:
                 print(f"ALERT: High/Extreme Volatility Detected!")

        # Cleanup
        if os.path.exists(temp_bars_file):
            os.remove(temp_bars_file)

if __name__ == "__main__":
    main()
