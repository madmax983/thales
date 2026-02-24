import subprocess
import json
import os
import sys
import re
from datetime import datetime

# Paths
CLI_PATH = "./target/debug/thales-cli"
PORTFOLIO_PATH = "portfolio.md"
STRATEGIES_PATH = "strategies.md"
HISTORY_PATH = "history.json"

def run_command(args):
    """Runs a thales-cli command and returns the parsed JSON data."""
    cmd = [CLI_PATH] + args
    try:
        # print(f"Running: {' '.join(cmd)}")
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        # Parse envelope
        try:
            envelope = json.loads(result.stdout)
            if envelope.get("status") == "ok":
                return envelope.get("data")
            else:
                print(f"Error executing {args}: {envelope.get('errors')}")
                return None
        except json.JSONDecodeError:
            print(f"Failed to parse JSON output from {args}")
            print(result.stdout)
            return None
    except subprocess.CalledProcessError as e:
        print(f"Command failed: {cmd}\nOutput: {e.output}\nError: {e.stderr}")
        return None

def get_active_strategy():
    """Parses strategies.md to find the active strategy name."""
    if not os.path.exists(STRATEGIES_PATH):
        print(f"Warning: {STRATEGIES_PATH} not found. Defaulting to BollingerBands.")
        return "BollingerBands"

    with open(STRATEGIES_PATH, "r") as f:
        content = f.read()

    strategies = []
    if "BollingerBandsMeanReversion" in content or "BollingerBands" in content:
        strategies.append("BollingerBands")
    if "EmaCrossover" in content:
        strategies.append("EmaCrossover")
    if "RsiMeanReversion" in content:
        strategies.append("RsiMeanReversion")

    # Pick the one that appears first in the file
    first_pos = float('inf')
    best_strategy = "BollingerBands"

    for s in strategies:
        pos = content.find(s)
        if pos != -1 and pos < first_pos:
            first_pos = pos
            best_strategy = s

    return best_strategy

def scan_markets():
    """Scans markets for candidates."""
    candidates = []

    # Crypto (Kraken)
    print("Scanning Kraken (Crypto)...")
    crypto = run_command(["scan-market", "--provider", "kraken", "--top-n", "10", "--min-volatility", "0.01", "--min-momentum", "0.0"])
    if crypto:
        for symbol in crypto:
            candidates.append({"provider": "kraken", "symbol": symbol, "market": "crypto"})

    # Equities (Alpaca)
    print("Scanning Alpaca (Equities)...")
    equities = run_command(["scan-market", "--provider", "alpaca"])
    if equities:
        for symbol in equities:
            candidates.append({"provider": "alpaca", "symbol": symbol, "market": "equities"})

    return candidates

def fetch_and_generate(candidate, strategy_name):
    """Fetches data and generates signal for a candidate."""
    provider = candidate["provider"]
    symbol = candidate["symbol"]

    # Fetch Data
    bars = run_command(["fetch-market-data", "--provider", provider, "--symbol", symbol, "--timeframe", "1h"])
    if not bars:
        return []

    # Save temp bars
    temp_bars_file = f"temp_bars_{symbol}.json"
    with open(temp_bars_file, "w") as f:
        json.dump(bars, f)

    # Generate Signals
    args = ["generate-signals", "--input", temp_bars_file, "--strategy", strategy_name]
    if os.path.exists(HISTORY_PATH):
        args.extend(["--history", HISTORY_PATH])

    intents = run_command(args)

    # Cleanup
    if os.path.exists(temp_bars_file):
        os.remove(temp_bars_file)

    if intents:
        # Enrich intent with provider for execution later
        for intent in intents:
            intent["provider"] = provider
        return intents
    return []

def log_trade(intent, result):
    """Logs executed trade to portfolio.md"""
    date_str = datetime.fromtimestamp(result["submitted_at_unix_ms"] / 1000).strftime("%Y-%m-%d %H:%M:%S")
    asset_class = intent["market"]
    symbol = intent["symbol"]
    action = intent["side"]
    size = intent["size_hint"]
    price = str(intent.get("limit_price", "Market"))
    if price == "None": price = "Market"

    sl = str(intent.get("stop_loss", "-"))
    if sl == "None": sl = "-"

    tp = str(intent.get("take_profit", "-"))
    if tp == "None": tp = "-"

    # Calc max risk if possible
    max_risk = "-"
    if sl != "-" and price != "Market":
        try:
             entry = float(price)
             stop = float(sl)
             qty = float(size)
             max_risk = f"{abs(entry - stop) * qty:.2f}"
        except:
             pass

    signal_ref = intent["intent_id"]
    rationale = intent["rationale"]

    line = f"| {date_str} | {asset_class} | {symbol} | {action} | {size} | {price} | {sl} | {tp} | {max_risk} | {signal_ref} | {rationale} |"

    with open(PORTFOLIO_PATH, "a") as f:
        f.write(line + "\n")

def log_skipped(intent, reason):
    """Logs skipped trade."""
    date_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    symbol = intent["symbol"]
    signal_ref = intent["intent_id"]

    line = f"| {date_str} | {symbol} | {signal_ref} | {reason} |"

    with open(PORTFOLIO_PATH, "a") as f:
         f.write(line + "\n")

def main():
    if not os.path.exists(CLI_PATH):
        print("Error: thales-cli not found. Run cargo build.")
        return

    # 1. Identify Strategy
    strategy_name = get_active_strategy()
    print(f"Active Strategy: {strategy_name}")

    # 2. Scan Markets
    candidates = scan_markets()
    print(f"Found {len(candidates)} candidates.")

    # 3. Generate Signals for all candidates
    all_signals = []
    print("Evaluating candidates...")
    for cand in candidates:
        signals = fetch_and_generate(cand, strategy_name)
        if signals:
            print(f"  {cand['symbol']}: Generated {len(signals)} signals.")
            all_signals.extend(signals)

    # 4. Filter and Select Top 3
    if not all_signals:
        print("No signals generated. Doing nothing.")
        return

    # Sort by confidence descending
    all_signals.sort(key=lambda x: x.get("confidence", 0.0), reverse=True)

    # Take top 3
    top_signals = all_signals[:3]

    print(f"Selected top {len(top_signals)} signals for execution.")

    # 5. Execute
    for intent in top_signals:
        provider = intent["provider"]
        print(f"Executing {intent['side']} {intent['symbol']} via {provider}...")

        # Write intent to file
        temp_intent_file = f"temp_intent_{intent['symbol']}.json"
        with open(temp_intent_file, "w") as f:
            json.dump(intent, f)

        # Execute
        result = run_command(["execute-intent", "--provider", provider, "--input", temp_intent_file])

        # Cleanup
        if os.path.exists(temp_intent_file):
            os.remove(temp_intent_file)

        if result:
            print(f"Success! Status: {result['status']}")
            log_trade(intent, result)
        else:
            print("Execution failed.")
            log_skipped(intent, "Execution Failed")

    # Log skipped signals (signals not selected in top 3)
    # Only if they were valid signals but we didn't select them.
    # We should log them as "Skipped" with reason "Lower priority/confidence".

    for intent in all_signals[3:]:
        log_skipped(intent, "Lower priority/confidence than top 3")

if __name__ == "__main__":
    main()
