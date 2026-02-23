import subprocess
import json
import os
import sys
import time
import re

def run_command(command, input_data=None):
    try:
        if input_data:
            process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            stdout, stderr = process.communicate(input=input_data)
        else:
            process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            stdout, stderr = process.communicate()

        if process.returncode != 0:
            print(f"Error running command: {' '.join(command)}")
            print(f"Stderr: {stderr}")
            return None
        return stdout
    except Exception as e:
        print(f"Exception running command: {e}")
        return None

def parse_envelope(output):
    try:
        start = output.find('{')
        if start == -1:
            return None
        json_str = output[start:]
        data = json.loads(json_str)
        if "data" in data:
            return data["data"]
        return data
    except json.JSONDecodeError:
        print(f"Failed to decode JSON output: {output[:100]}...")
        return None

def read_strategy_name():
    try:
        with open("strategies.md", "r") as f:
            content = f.read()
        if "BollingerBandsMeanReversion" in content:
            return "BollingerBandsMeanReversion"
        return "BollingerBands"
    except FileNotFoundError:
        return "BollingerBandsMeanReversion"

def read_signals_md_signals():
    """Extracts JSON blocks from Signals.md and returns potential buy/sell signals."""
    signals = []
    try:
        with open("Signals.md", "r") as f:
            content = f.read()

        # Robust extraction: look for {...} blocks, potentially multi-line
        # Using DOTALL to match newlines
        matches = re.findall(r'\{.*?\}', content, re.DOTALL)

        for match in matches:
            try:
                data = json.loads(match)
                if "symbol" in data and "sentiment" in data:
                    signals.append(data)
            except:
                continue

        print(f"Found {len(signals)} potential signals in Signals.md.")
        return signals
    except FileNotFoundError:
        print("Signals.md not found.")
        return signals

def main():
    # Expanded Universe to simulate market scan
    # Note: Using Alpaca for equities because thales-cli maps equities to alpaca only.
    equities = ["SPY", "QQQ", "TQQQ", "AAPL", "NVDA", "TSLA", "AMZN", "META", "MSFT", "AMD", "GOOGL"]
    # 20+ Crypto pairs
    crypto = [
        "XBTUSD", "ETH/USD", "SOL/USD", "LTC/USD", "BNB/USD", "XRP/USD", "ADA/USD", "DOGE/USD",
        "DOT/USD", "MATIC/USD", "SHIB/USD", "TRX/USD", "AVAX/USD", "LINK/USD", "UNI/USD",
        "XLM/USD", "ATOM/USD", "ETC/USD", "FIL/USD", "XMR/USD"
    ]

    universe = []
    for s in equities:
        universe.append({"symbol": s, "provider": "alpaca", "market": "equities"})
    for s in crypto:
        universe.append({"symbol": s, "provider": "kraken", "market": "crypto"})

    cli_cmd = ["cargo", "run", "-q", "-p", "thales-cli", "--"]
    strategy_name = read_strategy_name()
    print(f"Active Strategy: {strategy_name}")

    signals_from_md = read_signals_md_signals()

    # 1. Scan & Calculate Metrics (24h / Daily)
    candidates = []
    processed_symbols = set()

    print(f"Scanning market ({len(universe)} assets) using 1d bars...")
    for asset in universe:
        symbol = asset["symbol"]
        provider = asset["provider"]

        # Fetch 1d bars for metrics calculation
        fetch_cmd = cli_cmd + ["fetch-market-data", "--provider", provider, "--symbol", symbol, "--timeframe", "1d"]
        fetch_output = run_command(fetch_cmd)
        if not fetch_output: continue

        # Normalize
        safe_symbol = symbol.replace("/", "_")
        temp_bars_file = f"temp_bars_{safe_symbol}.json"
        with open("temp_fetch.json", "w") as f:
            f.write(fetch_output)
        norm_cmd = cli_cmd + ["normalize-bars", "--input", "temp_fetch.json"]
        norm_output = run_command(norm_cmd)
        if not norm_output: continue

        # Parse bars
        bars_data = parse_envelope(norm_output)
        if not bars_data or "bars" not in bars_data or not bars_data["bars"]:
            continue

        bars = bars_data["bars"]
        last_bar = bars[-1]

        # Metrics
        volatility = (last_bar["high"] - last_bar["low"]) / last_bar["open"]
        momentum = (last_bar["close"] - last_bar["open"]) / last_bar["open"]
        volume = last_bar["volume"]

        candidates.append({
            "asset": asset,
            "volatility": volatility,
            "momentum": momentum,
            "volume": volume,
            "bars_file": temp_bars_file
        })

        with open(temp_bars_file, "w") as f:
            f.write(norm_output)

    # Sort/Select

    crypto_candidates = [c for c in candidates if c["asset"]["market"] == "crypto"]
    equity_candidates = [c for c in candidates if c["asset"]["market"] == "equities"]

    # Crypto: Top 10 by Volume
    crypto_candidates.sort(key=lambda x: x["volume"], reverse=True)
    top_crypto_vol = crypto_candidates[:10]

    # Filter Top 10 by Volatility -> Top 3
    top_crypto_vol.sort(key=lambda x: x["volatility"], reverse=True)
    top_crypto = top_crypto_vol[:3]

    # Equity: Top 3 by Momentum
    equity_candidates.sort(key=lambda x: abs(x["momentum"]), reverse=True)
    top_equity = equity_candidates[:3]

    # Final Selection
    all_top = top_crypto + top_equity
    all_top.sort(key=lambda x: x["volatility"], reverse=True)
    final_candidates = all_top[:3]

    print(f"Selected Top Candidates: {[c['asset']['symbol'] for c in final_candidates]}")

    executed_count = 0

    # Process Signals.md Candidates
    for sig in signals_from_md:
        symbol = sig.get("symbol")
        sentiment = sig.get("sentiment", "Neutral")

        # Find asset config in universe or create
        asset_config = next((a for a in universe if a["symbol"] == symbol), None)
        if not asset_config:
            # Try to guess provider
            market = sig.get("market", "equities") # default
            provider = "kraken" if market == "crypto" else "alpaca"
            asset_config = {"symbol": symbol, "provider": provider, "market": market}

        # Find candidate entry if available (to reuse fetched data)
        cand = next((c for c in candidates if c["asset"]["symbol"] == symbol), None)

        if cand:
            print(f"Validating Signals.md entry for {symbol} ({sentiment}) against active strategy...")
            executed = process_candidate(cand, cli_cmd, strategy_name, sentiment_bias=sentiment)
            if executed: executed_count += 1
            processed_symbols.add(symbol)
        else:
            # Need to fetch if not scanned
            pass

    # Process Top Candidates (if not already processed)
    for cand in final_candidates:
        symbol = cand["asset"]["symbol"]
        if symbol in processed_symbols: continue

        print(f"Analyzing Top Candidate {symbol}...")
        executed = process_candidate(cand, cli_cmd, strategy_name)
        if executed: executed_count += 1
        processed_symbols.add(symbol)

    # Cleanup
    for cand in candidates:
        try:
            os.remove(cand["bars_file"])
        except:
            pass
    try:
        os.remove("temp_fetch.json")
        os.remove("temp_intent.json")
    except:
        pass

    if executed_count == 0:
        print("No trades executed.")

def process_candidate(cand, cli_cmd, strategy_name, sentiment_bias=None):
    asset = cand["asset"]
    symbol = asset["symbol"]
    provider = asset["provider"]
    bars_file = cand["bars_file"]

    # Generate Signals
    gen_cmd = cli_cmd + ["generate-signals", "--input", bars_file, "--strategy", strategy_name, "--history", "history.json"]
    gen_output = run_command(gen_cmd)
    if not gen_output: return False

    intents = parse_envelope(gen_output)
    if not intents:
        if sentiment_bias:
            log_rejection(symbol, "Signals.md", f"Strategy {strategy_name} returned no signal despite {sentiment_bias} sentiment.")
        return False

    for intent in intents:
        side = intent.get("side")
        if sentiment_bias:
            if (sentiment_bias == "Bullish" and side != "buy") or (sentiment_bias == "Bearish" and side != "sell"):
                reason = f"Signal mismatch: Signals.md {sentiment_bias} vs Strategy {side}."
                print(f"{reason} Skipping.")
                log_rejection(symbol, "Signals.md", reason)
                continue

        # Write intent
        with open("temp_intent.json", "w") as f:
            json.dump(intent, f)

        # Validate
        val_cmd = cli_cmd + ["validate-intent", "--input", "temp_intent.json"]
        val_output = run_command(val_cmd)
        if not val_output: continue
        val_result = parse_envelope(val_output)
        if not val_result or not val_result.get("valid"):
            reason = "Intent failed validation."
            print(f"Intent invalid for {symbol}.")
            log_rejection(symbol, "Generated", reason)
            continue

        # Execute
        print(f"Executing trade for {symbol}...")
        exec_cmd = cli_cmd + ["execute-intent", "--provider", provider, "--input", "temp_intent.json"]
        exec_output = run_command(exec_cmd)
        if not exec_output:
            print("Execution failed.")
            continue

        exec_result = parse_envelope(exec_output)
        if not exec_result:
            print("Failed to parse execution result.")
            continue

        print(f"Execution successful: {exec_result['provider_order_id']}")
        log_portfolio(intent, exec_result, signal_ref="Signals.md" if sentiment_bias else "Market Scan")
        return True
    return False

def log_rejection(symbol, signal_ref, reason):
    log_path = "trade_rejections.md"
    date_str = time.strftime("%Y-%m-%d %H:%M:%S")

    # Format: | Date/Time | Symbol | Signal Ref | Rejection Reason |
    row = f"| {date_str} | {symbol} | {signal_ref} | {reason} |"

    with open(log_path, "a") as f:
        # Check if empty, add header
        if os.stat(log_path).st_size == 0:
            f.write("| Date/Time | Symbol | Signal Ref | Rejection Reason |\n")
            f.write("|---|---|---|---|\n")
        f.write(f"{row}\n")

def log_portfolio(intent, exec_result, signal_ref="N/A"):
    portfolio_path = "portfolio.md"
    date_str = time.strftime("%Y-%m-%d %H:%M:%S")
    market = intent.get("market", "unknown")
    symbol = intent.get("symbol", "unknown")
    side = intent.get("side", "unknown")
    size_str = intent.get("size_hint", "0")

    try:
        size = float(size_str)
    except:
        size = 0.0

    price_val = 0.0
    price_str = "market"

    if intent.get("limit_price"):
        price_val = float(intent["limit_price"])
        price_str = str(price_val)
    elif intent.get("stop_price"):
        price_val = float(intent["stop_price"])
        price_str = str(price_val)
    else:
        pass

    sl = intent.get("stop_loss", "N/A")
    tp = intent.get("take_profit", "N/A")

    max_risk = "N/A"

    if sl != "N/A" and price_str != "market":
        try:
            sl_val = float(sl)
            risk_per_unit = abs(price_val - sl_val)
            total_risk = risk_per_unit * size
            max_risk = f"{total_risk:.2f}"
        except:
            pass
    elif sl != "N/A":
        max_risk = "~100.0"

    rationale = intent.get("rationale", "N/A")

    # If signal_ref was passed, use it, otherwise use intent_id
    ref = signal_ref if signal_ref != "N/A" else intent.get("intent_id", "N/A")

    row = f"| {date_str} | {market} | {symbol} | {side} | {size_str} | {price_str} | {sl} | {tp} | {max_risk} | {ref} | {rationale} |"

    # Check if portfolio needs header (e.g. if we are introducing new columns or if empty)
    # The current portfolio.md has a different header.
    # We are appending.
    # To be safe, we can check if file is empty.

    exists = os.path.exists(portfolio_path)

    with open(portfolio_path, "a") as f:
        if not exists or os.stat(portfolio_path).st_size == 0:
             f.write("| Date/Time | Asset Class | Symbol/Contract | Action | Size/Qty | Entry Price | SL | TP | Max Risk | Signal Ref | Rationale |\n")
             f.write("|---|---|---|---|---|---|---|---|---|---|---|\n")
        f.write(f"\n{row}")

if __name__ == "__main__":
    # Ensure log file exists or handle in function
    if not os.path.exists("trade_rejections.md"):
        open("trade_rejections.md", "w").close()
    main()
