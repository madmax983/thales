import subprocess
import json
import os
import sys
import re
import shutil
import math
from datetime import datetime

# Paths
CLI_PATH = "./target/debug/thales-cli"
PORTFOLIO_PATH = "portfolio.md"
STRATEGIES_PATH = "strategies.md"
HISTORY_PATH = "history.json"
SIGNALS_PATH = "Signals.md"

def run_command(args):
    """Runs a thales-cli command and returns the parsed JSON data."""
    cmd = [CLI_PATH] + args
    try:
        # print(f"Running: {' '.join(cmd)}")
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        # Parse envelope
        output = result.stdout.strip()
        # Attempt to find JSON start
        json_start = output.find('{')
        if json_start != -1:
            json_str = output[json_start:]
            try:
                envelope = json.loads(json_str)
                if envelope.get("status") == "ok":
                    return envelope.get("data")
                else:
                    print(f"Error executing {args}: {envelope.get('errors')}")
                    return None
            except json.JSONDecodeError:
                pass # Fall through to error reporting

        print(f"Failed to parse JSON output from {args}")
        print(result.stdout)
        return None
    except subprocess.CalledProcessError as e:
        print(f"Command failed: {cmd}\nOutput: {e.output}\nError: {e.stderr}")
        return None

def get_active_strategy():
    """Parses strategies.md to find the active strategy name."""
    if not os.path.exists(STRATEGIES_PATH):
        print(f"Warning: {STRATEGIES_PATH} not found.")
        return None

    with open(STRATEGIES_PATH, "r") as f:
        content = f.read()

    strategies = []
    if "BollingerBandsMeanReversion" in content or "BollingerBands" in content:
        strategies.append("BollingerBands")
    if "EmaCrossover" in content:
        strategies.append("EmaCrossover")
    if "RsiMeanReversion" in content:
        strategies.append("RsiMeanReversion")
    if "Macd" in content:
        strategies.append("Macd")

    if not strategies:
        return None

    # Pick the one that appears first in the file
    first_pos = float('inf')
    best_strategy = None

    for s in strategies:
        pos = content.find(s)
        if pos != -1 and pos < first_pos:
            first_pos = pos
            best_strategy = s

    return best_strategy

def get_candidates_from_signals():
    """Parses Signals.md for potential candidates."""
    if not os.path.exists(SIGNALS_PATH):
        return []

    with open(SIGNALS_PATH, "r") as f:
        content = f.read()

    candidates = {}

    # New parsing logic to handle sections and extract full context
    chunks = re.split(r"\n## ", content)
    for chunk in chunks:
        chunk = chunk.strip()
        if not chunk: continue

        market = None
        symbol = None

        # Format 1: Market Analysis Report - <market> - <symbol>
        match1 = re.match(r"Market Analysis Report - (\w+) - (\w+)", chunk)
        if match1:
            market = match1.group(1)
            symbol = match1.group(2)

        # Format 2: Symbol: <symbol> (<market>)
        if not symbol:
            match2 = re.match(r"Symbol: (\w+) \((\w+)\)", chunk)
            if match2:
                symbol = match2.group(1)
                market = match2.group(2)

        if not symbol:
            continue

        provider = "kraken" if market == "crypto" else "alpaca"

        # Extract JSON
        json_match = re.search(r"```json\s*(\{.*?\})\s*```", chunk, re.DOTALL)
        raw_json = None
        if json_match:
            try:
                raw_json = json.loads(json_match.group(1))
            except:
                pass

        # Extract Research
        research_text = None
        res_match = re.search(r"\*\*Research\*\*:\s*(.*?)(?=\n\n|\n\*\*|\n###|$)", chunk, re.DOTALL)
        if res_match:
            research_text = res_match.group(1).strip()
        else:
             res_match_alt = re.search(r"\*External Research\*:\s*(.*?)(?=\n\n|\n\*\*|\n###|$)", chunk, re.DOTALL)
             if res_match_alt:
                 research_text = res_match_alt.group(1).strip()

        # Extract News
        news_text = None
        news_match = re.search(r"\*\*News\*\*:\s*(.*?)(?=\n\n|\n\*\*|\n###|$)", chunk, re.DOTALL)
        if news_match:
            news_text = news_match.group(1).strip()

        if raw_json:
            if research_text:
                raw_json["research_summary"] = research_text
            if news_text:
                raw_json["news_summary"] = news_text

        candidates[symbol] = {
            "provider": provider,
            "symbol": symbol,
            "market": market,
            "raw_analysis_json": raw_json
        }

    return list(candidates.values())

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

    # Generate Analysis (for history)
    analysis = run_command(["analyze-market", "--input", temp_bars_file, "--no-report"])

    # Generate Signals
    args = ["generate-signals", "--input", temp_bars_file, "--strategy", strategy_name]
    if os.path.exists(HISTORY_PATH):
        args.extend(["--history", HISTORY_PATH])

    # NEW: Pass enriched analysis if available
    temp_analysis_file = None
    if candidate.get("raw_analysis_json"):
        temp_analysis_file = f"temp_analysis_{symbol}.json"
        with open(temp_analysis_file, "w") as f:
            json.dump(candidate["raw_analysis_json"], f)
        args.extend(["--analysis", temp_analysis_file])

    intents = run_command(args)

    # Cleanup
    if os.path.exists(temp_bars_file):
        os.remove(temp_bars_file)
    if temp_analysis_file and os.path.exists(temp_analysis_file):
        os.remove(temp_analysis_file)

    if intents:
        # Enrich intent with provider for execution later
        for intent in intents:
            intent["provider"] = provider
            # If we used raw_analysis_json, it's already "baked into" the signal rationale.
            # But we might still want to attach it for history.
            if analysis:
                intent["_market_analysis"] = analysis
            elif candidate.get("raw_analysis_json"):
                 intent["_market_analysis"] = candidate["raw_analysis_json"]
        return intents
    return []

def update_history(intent):
    """Appends executed trade to history.json."""
    if "_market_analysis" not in intent:
        return

    analysis = intent["_market_analysis"]
    # Clean up internal field before saving? Or keep it separate.
    # We need to construct HistoryEntry: { intent, market_analysis, outcome }

    # Create a clean intent copy without internal fields
    clean_intent = intent.copy()
    if "_market_analysis" in clean_intent:
        del clean_intent["_market_analysis"]
    if "provider" in clean_intent: # provider is also internal
        del clean_intent["provider"]

    entry = {
        "intent": clean_intent,
        "market_analysis": analysis,
        "outcome": None
    }

    history = []
    if os.path.exists(HISTORY_PATH):
        try:
            with open(HISTORY_PATH, "r") as f:
                history = json.load(f)
        except Exception as e:
            print(f"Warning: Failed to load history.json: {e}")
            # Backup corrupted file
            timestamp = datetime.now().strftime("%Y%m%d%H%M%S")
            backup_path = f"{HISTORY_PATH}.bak.{timestamp}"
            try:
                shutil.copy(HISTORY_PATH, backup_path)
                print(f"Backed up corrupted history to {backup_path}")
            except Exception as copy_err:
                print(f"Failed to backup corrupted history: {copy_err}")
            history = []

    history.append(entry)

    with open(HISTORY_PATH, "w") as f:
        json.dump(history, f, indent=2)

def log_trade(intent, result):
    """Logs executed trade to portfolio.md"""
    date_str = datetime.fromtimestamp(result["submitted_at_unix_ms"] / 1000).strftime("%Y-%m-%d %H:%M:%S")
    asset_class = intent.get("market", "-")
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

def verify_risk(intent):
    """
    Risk Agent logic to verify trade intent before execution.
    Returns (bool, reason).
    """
    # Defensive checks
    if not isinstance(intent, dict):
        return False, "Invalid intent format"

    symbol = intent.get("symbol", "Unknown")
    side = intent.get("side", "unknown")
    signal_type = intent.get("signal_type", "Unknown")
    size_hint = intent.get("size_hint", "0")
    stop_loss = intent.get("stop_loss")
    confidence = intent.get("confidence", 0.0)

    # 1. Check Size
    if size_hint == "max":
        # Valid for Exit/ScaleOut
        pass
    else:
        try:
            size = float(size_hint)
            if math.isnan(size):
                 return False, f"Invalid size: NaN"
            if math.isinf(size):
                 return False, f"Invalid size: Infinity"
            if size <= 0:
                return False, f"Invalid size: {size_hint} (must be > 0)"
        except ValueError:
            return False, f"Invalid size format: {size_hint}"

    # 2. Check Stop Loss for Entries
    # Signal type might be "SignalType::Entry" string from Rust debug format
    # Or just "Entry"
    is_entry = False
    if signal_type:
        if "Entry" in signal_type or "ScaleIn" in signal_type:
            is_entry = True

    if is_entry:
        if stop_loss is None:
             return False, "Missing Stop Loss for Entry"

    # 3. Check Confidence
    if confidence < 0.5:
        return False, f"Low confidence: {confidence}"

    return True, "Approved"

def main():
    if not os.path.exists(CLI_PATH):
        print("Error: thales-cli not found. Run cargo build.")
        return

    # 1. Identify Strategy
    strategy_name = get_active_strategy()
    if not strategy_name:
        print("No active strategy found in strategies.md. Doing nothing.")
        # Log why?
        with open(PORTFOLIO_PATH, "a") as f:
             f.write(f"\n# Execution Attempt {datetime.now()}\nNo active strategy found. Aborting.\n")
        return

    print(f"Active Strategy: {strategy_name}")

    # 2. Scan Markets + Get from Signals.md
    scanned_candidates = scan_markets()
    signal_candidates = get_candidates_from_signals()

    # Merge candidates (prefer signal candidates if duplicates?)
    # Using a dict to deduplicate by symbol
    candidates_map = {c["symbol"]: c for c in scanned_candidates}
    for c in signal_candidates:
        candidates_map[c["symbol"]] = c # Overwrite or add

    candidates = list(candidates_map.values())
    print(f"Found {len(candidates)} unique candidates (Scanned: {len(scanned_candidates)}, Signals: {len(signal_candidates)}).")

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
        # Risk Agent Check
        risk_ok, risk_reason = verify_risk(intent)
        if not risk_ok:
            print(f"Skipping {intent['symbol']}: {risk_reason}")
            log_skipped(intent, f"Rejected by Risk Agent: {risk_reason}")
            continue

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
            # Handle list response from execute-intent
            exec_res = result[0] if isinstance(result, list) else result
            print(f"Success! Status: {exec_res.get('status')}")
            log_trade(intent, exec_res)
            update_history(intent)
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
