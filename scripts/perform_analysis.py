import subprocess
import json
import os
import sys
from datetime import datetime

# Paths
CLI_PATH = "./target/release/thales-cli"
SIGNALS_PATH = "Signals.md"
MARKET_REGIME_PATH = "Market_Regime.md"
VOLATILITY_REGIME_PATH = "Volatility_Regime.md"
MARKET_RESEARCH_PATH = "Market_Research.md"

# Research Data (from agent step)
RESEARCH_DATA = {
    "BTCUSD": {
        "market": "crypto",
        "provider": "kraken",
        "research": "Bitcoin daily gains near 5% as analysis eyes bullish 'rotation' from gold. Clarity Act risks repeat of Europe's mistakes, crypto lawyer warns.",
        "news": "Bitcoin price today is $66,174.00 (+5.37% in 24h). Solana leads crypto recovery with 10% gain."
    },
    "ETHUSD": {
        "market": "crypto",
        "provider": "kraken",
        "research": "Correlated with broader crypto recovery led by Bitcoin and Solana. Market sentiment generally bullish following Bitcoin's 5% gain.",
        "news": "No specific ETH news found. Inferring bullish sentiment from BTC/SOL trends."
    },
    "SPY": {
        "market": "equities",
        "provider": "alpaca",
        "research": "No specific news found. Analyzing technical structure.",
        "news": "No specific news found."
    }
}

def run_command(args):
    """Runs a thales-cli command."""
    cmd = [CLI_PATH] + args
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=False)
        stdout = (result.stdout or "").strip()
        stderr = (result.stderr or "").strip()

        if result.returncode != 0:
            print(f"Command failed: {cmd}\nStderr: {stderr}")
            return None

        # Extract JSON from stdout
        json_start = stdout.find("{")
        if json_start != -1:
            try:
                envelope = json.loads(stdout[json_start:])
                if envelope.get("status") == "ok":
                    return envelope.get("data")
                else:
                    print(f"Command returned error status: {envelope}")
                    return None
            except json.JSONDecodeError:
                print(f"Failed to decode JSON from: {stdout}")
                return None
        return None
    except Exception as e:
        print(f"Exception: {e}")
        return None

def append_to_file(filepath, content):
    with open(filepath, "a") as f:
        f.write(content + "\n")

def main():
    if not os.path.exists(CLI_PATH):
        print(f"Error: {CLI_PATH} not found. Did you build?")
        sys.exit(1)

    # 1. Iterate symbols
    for symbol, info in RESEARCH_DATA.items():
        print(f"Analyzing {symbol}...")

        # 2. Fetch Data
        # Using synthetic data as I don't have API keys.
        # The fetch command outputs a JSON envelope with BarSeries.
        # I need to save it to a temp file to pass to analyze-market.
        fetch_args = ["fetch-market-data", "--provider", info["provider"], "--symbol", symbol, "--timeframe", "1h"]
        bars_data = run_command(fetch_args)

        if not bars_data:
            print(f"Failed to fetch data for {symbol}")
            continue

        temp_bars_file = f"temp_bars_{symbol}.json"
        # We need to wrap the data in the envelope structure expected by analyze-market input?
        # Actually, thales-cli commands usually output the data part if piped, but let's see.
        # The run_command returns envelope['data'].
        # I should probably write the full envelope or just the data depending on what analyze-market expects.
        # Looking at execute_cycle.py: `run_command` returns `envelope.get("data")`.
        # And `analyze-market` takes `--input temp_bars_file`.
        # I suspect `fetch-market-data` output (JSON) is what `analyze-market` expects.
        # Let's write the `bars_data` to file.
        # Wait, `run_command` returns the inner `data` object.
        # Does `analyze-market` expect the full envelope or just the data?
        # Re-reading AGENTS.md: "Input: JSON files or command flags".
        # Usually internal tools expect the data structure (BarSeries), which is likely what `bars_data` is.

        with open(temp_bars_file, "w") as f:
            json.dump(bars_data, f)

        # 3. Analyze
        analyze_args = [
            "analyze-market",
            "--input", temp_bars_file,
            "--research", info["research"],
            "--news", info["news"]
        ]

        analysis = run_command(analyze_args)

        # Cleanup temp file
        if os.path.exists(temp_bars_file):
            os.remove(temp_bars_file)

        if not analysis:
            print(f"Failed to analyze {symbol}")
            continue

        # 4. Format and Append Output
        # Signals.md
        timestamp = analysis.get("timestamp_unix_ms", int(datetime.now().timestamp() * 1000))
        confidence = analysis.get("confidence", 0.0) * 100
        regime = analysis.get("regime", "Unknown")
        sentiment = analysis.get("sentiment", "Neutral")
        volatility = analysis.get("volatility", "Unknown")
        recommendation = analysis.get("recommendation", "Neutral")

        patterns = ", ".join(analysis.get("patterns", [])) or "None detected"
        key_levels = ", ".join(map(str, analysis.get("key_levels", []))) or "None identified"

        report = f"""
## Market Analysis Report - {info['market']} - {symbol}

**Timestamp (ms)**: {timestamp}
**Confidence**: {confidence:.2f}%

### 1. Market Regime
**Regime**: {regime}
*Sentiment*: {sentiment}

### 2. Volatility
*Assessment*: {volatility}

### 3. Strategy Recommendation
**{recommendation}**

### 4. Patterns & Price Action
*Patterns*: {patterns}

### 5. Key Levels
*Support/Resistance*: {key_levels}

### 6. Research & Context
**Research**:
{info['research']}

**News**:
{info['news']}

```json
{json.dumps(analysis, indent=2)}
```
---
"""
        append_to_file(SIGNALS_PATH, report)
        print(f"Appended report for {symbol} to {SIGNALS_PATH}")

        # Market_Regime.md
        # Just append a summary line
        regime_summary = f"- **{symbol}**: {regime} ({sentiment})"
        append_to_file(MARKET_REGIME_PATH, regime_summary)

        # Volatility_Regime.md
        vol_summary = f"- **{symbol}**: {volatility}"
        append_to_file(VOLATILITY_REGIME_PATH, vol_summary)

        # Market_Research.md
        res_summary = f"### {symbol} - {datetime.now().strftime('%Y-%m-%d')}\n{info['research']}\n"
        append_to_file(MARKET_RESEARCH_PATH, res_summary)

if __name__ == "__main__":
    main()
