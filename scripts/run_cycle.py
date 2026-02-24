import subprocess
import json
import os
import sys
import re
from datetime import datetime

CLI_PATH = "./target/debug/thales-cli"
PORTFOLIO_PATH = "portfolio.md"
STRATEGIES_PATH = "strategies.md"
SIGNALS_PATH = "Signals.md"
HISTORY_PATH = "history.json"

def run_command(args):
    """Runs a thales-cli command and returns the parsed JSON data."""
    cmd = [CLI_PATH] + args
    run_command.last_error = None
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=False)
        stdout = (result.stdout or "").strip()
        stderr = (result.stderr or "").strip()

        envelope = None
        if stdout:
            try:
                envelope = json.loads(stdout)
            except json.JSONDecodeError:
                envelope = None

        if envelope and envelope.get("status") == "ok":
            return envelope.get("data")

        if envelope and envelope.get("status") == "error":
            errors = envelope.get("errors") or []
            reason = "; ".join(str(e) for e in errors) if errors else "Execution failed"
            run_command.last_error = reason
            print(f"Error executing {args}: {reason}")
            return None

        if result.returncode != 0:
            run_command.last_error = stderr or stdout or f"Command failed with exit code {result.returncode}"
            print(f"Command failed: {cmd}\nReason: {run_command.last_error}")
            return None

        run_command.last_error = "Unexpected CLI response format."
        print(f"Failed to parse CLI response from {args}: {stdout[:200]}")
        return None
    except Exception as e:
        run_command.last_error = str(e)
        print(f"Exception running command {cmd}: {e}")
        return None

run_command.last_error = None

def log_trade(intent, execution_result):
    """Logs a successful trade to portfolio.md."""
    date_str = datetime.fromtimestamp(execution_result["submitted_at_unix_ms"] / 1000).strftime("%Y-%m-%d %H:%M:%S")
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

    # Calculate Max Risk (approximate if SL exists)
    max_risk = "-"
    if sl != "-" and price != "Market":
        try:
            entry = float(price)
            stop = float(sl)
            qty = float(size)
            risk = abs(entry - stop) * qty
            max_risk = f"{risk:.2f}"
        except:
            pass

    signal_ref = intent["intent_id"]
    rationale = intent["rationale"]

    line = f"| {date_str} | {asset_class} | {symbol} | {action} | {size} | {price} | {sl} | {tp} | {max_risk} | {signal_ref} | {rationale} |"

    with open(PORTFOLIO_PATH, "a") as f:
        f.write(line + "\n")

def log_rejection(symbol, signal_ref, reason):
    """Logs a rejected trade."""
    date_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    line = f"| {date_str} | {symbol} | {signal_ref} | {reason} |"

    with open("rejected_trades.md", "a") as f:
        if not os.path.exists("rejected_trades.md") or os.stat("rejected_trades.md").st_size == 0:
             f.write("| Date/Time | Symbol | Signal Ref | Rejection Reason |\n|---|---|---|---|\n")
        f.write(line + "\n")

def get_active_strategy():
    """Parses strategies.md to find the active strategy name."""
    if not os.path.exists(STRATEGIES_PATH):
        print(f"Warning: {STRATEGIES_PATH} not found. Defaulting to BollingerBands.")
        return "BollingerBands"

    with open(STRATEGIES_PATH, "r") as f:
        content = f.read()

    # Look for "Trading Strategy: Name" or similar
    # strategies.md format: "# Trading Strategy: Bollinger Bands Mean Reversion"
    # Also "Name: BollingerBandsMeanReversion"

    match = re.search(r"\*\*Name:\*\*\s+(\w+)", content)
    if match:
        return match.group(1)

    match = re.search(r"# Trading Strategy:\s+(.+)", content)
    if match:
        # Clean up name to match code expectation if needed.
        # Code checks "BollingerBands" or "BollingerBandsMeanReversion".
        name = match.group(1).strip()
        if "Bollinger Bands" in name:
            return "BollingerBands"
        return name

    return "BollingerBands"

def get_signal_candidates():
    """Parses Signals.md for symbols to process."""
    if not os.path.exists(SIGNALS_PATH):
        return []

    candidates = []
    with open(SIGNALS_PATH, "r") as f:
        content = f.read()

    # Look for "## Symbol: AAPL (equities)" or "Symbol: AAPL"
    # Regex for "Symbol: (\w+)"
    matches = re.findall(r"Symbol:\s+([A-Z0-9/]+)", content)

    # Also need provider/market. infer based on symbol or look for market.
    # signals.md has "## Symbol: AAPL (equities)"

    for symbol in matches:
        # Normalize symbol (remove / for kraken pairs if raw)
        # But we need provider.
        # Try to infer or find context.
        # Assuming simple lookup: if ends in USD/ETH/BTC -> Kraken, else Alpaca.
        # Or check if "equities" or "crypto" is near.

        provider = "alpaca" # Default
        if "USD" in symbol or "XBT" in symbol or "ETH" in symbol:
             provider = "kraken"

        # Look for explicit market in file content nearby? Too complex for regex.
        # Let's rely on basic inference or check scan results.

        candidates.append((provider, symbol))

    return list(set(candidates)) # Dedupe

def process_symbol(provider, symbol, strategy_name):
    print(f"Processing {symbol} ({provider})...")

    # Fetch Data
    bars = run_command(["fetch-market-data", "--provider", provider, "--symbol", symbol, "--timeframe", "1h"])
    if not bars:
        print(f"Failed to fetch data for {symbol}")
        return False

    with open("temp_bars.json", "w") as f:
        json.dump(bars, f)

    # Fetch Positions (for redundancy check)
    positions = run_command(["get-positions", "--provider", provider])
    with open("temp_positions.json", "w") as f:
        json.dump(positions or [], f)

    # Generate Signals (Validation)
    intents = run_command(["generate-signals", "--input", "temp_bars.json", "--strategy", strategy_name, "--history", HISTORY_PATH, "--portfolio", "temp_positions.json"])

    if not intents:
        print(f"No signals for {symbol}")
        return False

    executed = False
    for intent in intents:
        print(f"Signal generated: {intent['side']} {intent['symbol']}")

        with open("temp_intent.json", "w") as f:
            json.dump(intent, f)

        result = run_command(["execute-intent", "--provider", provider, "--input", "temp_intent.json"])

        if result:
            print(f"Execution successful: {result['status']}")
            log_trade(intent, result)
            executed = True
        else:
            reason = run_command.last_error or "Execution failed"
            print(f"Execution failed: {reason}")
            log_rejection(symbol, intent["intent_id"], reason)

    return executed

def main():
    if not os.path.exists(CLI_PATH):
        print(f"Error: {CLI_PATH} not found. Build the project first.")
        return

    strategy_name = get_active_strategy()
    print(f"Active Strategy: {strategy_name}")

    # 1. Consumer Mode: Check Signals.md
    print("Checking Signals.md for candidates...")
    signal_candidates = get_signal_candidates()

    executed_signals = False
    if signal_candidates:
        print(f"Found candidates in Signals.md: {signal_candidates}")
        for provider, symbol in signal_candidates:
            if process_symbol(provider, symbol, strategy_name):
                executed_signals = True
    else:
        print("No candidates found in Signals.md.")

    # 2. Generator Mode: Scan Markets (Fallback or Complementary?)
    # Prompt says: "If a signal from signals.md is validated ... execute... If there are no signals, and you can create a valid one, make a trade."
    # So if we didn't execute anything from Signals.md (either empty or validation failed), we scan.

    if not executed_signals:
        print("No valid signals executed from Signals.md. Scanning markets...")

        print("Scanning Crypto Markets (Kraken)...")
        # Added min-momentum argument
        crypto_symbols = run_command(["scan-market", "--provider", "kraken", "--top-n", "10", "--min-volatility", "0.01", "--min-momentum", "0.0"]) or []
        print(f"Found {len(crypto_symbols)} crypto candidates: {crypto_symbols}")

        print("Scanning Equity Markets (Alpaca)...")
        equity_symbols = run_command(["scan-market", "--provider", "alpaca"]) or []
        print(f"Found {len(equity_symbols)} equity candidates: {equity_symbols}")

        all_symbols = [("kraken", s) for s in crypto_symbols] + [("alpaca", s) for s in equity_symbols]

        for provider, symbol in all_symbols:
            process_symbol(provider, symbol, strategy_name)

    # Clean up
    if os.path.exists("temp_bars.json"): os.remove("temp_bars.json")
    if os.path.exists("temp_intent.json"): os.remove("temp_intent.json")
    if os.path.exists("temp_positions.json"): os.remove("temp_positions.json")

if __name__ == "__main__":
    main()
