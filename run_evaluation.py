import json
import subprocess
import os
import datetime

# Configuration
CANDIDATES = [
    {"symbol": "COQUSD", "provider": "kraken", "market": "crypto"},
    {"symbol": "REKTUSD", "provider": "kraken", "market": "crypto"},
    {"symbol": "PEPEUSD", "provider": "kraken", "market": "crypto"},
    {"symbol": "SPY", "provider": "alpaca", "market": "equities"},
    {"symbol": "QQQ", "provider": "alpaca", "market": "equities"},
    {"symbol": "TQQQ", "provider": "alpaca", "market": "equities"},
]

# We want to pick the top 1-3 candidates overall. To maintain diversification, let's select:
# 1. SPY (Already holding position, need to evaluate to hold/sell/scale-in)
# 2. COQUSD (Top Crypto candidate)
# 3. REKTUSD (2nd Crypto candidate)
SELECTED_CANDIDATES = [
    {"symbol": "SPY", "provider": "alpaca", "market": "equities"},
    {"symbol": "COQUSD", "provider": "kraken", "market": "crypto"},
    {"symbol": "REKTUSD", "provider": "kraken", "market": "crypto"},
]

STRATEGIES = [
    "BollingerBands", "EmaCrossover", "RsiMeanReversion", "Macd", "Supertrend",
    "DonchianBreakout", "ParabolicSar", "KeltnerChannelBreakout", "StochasticOscillator",
    "AdxMomentum", "IchimokuCloud", "CciMomentum", "LinearRegressionTrend", "ObvTrendFollowing",
    "MoneyFlowIndex", "ConnorsRsiMeanReversion", "AwesomeOscillator", "WilliamsR",
    "VwmaCrossover", "VwapReversion", "VortexBreakout", "ZScoreMeanReversion",
    "ChaikinMoneyFlow", "ElderRay", "ChandelierExit", "AroonOscillator", "RocMomentum",
    "StochRsiMeanReversion"
]

def run_cmd(cmd):
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    if result.returncode != 0:
        return None
    try:
        data = json.loads(result.stdout)
        if data.get("status") == "ok":
            return data.get("data")
    except (json.JSONDecodeError, ValueError, TypeError, OSError, IndexError, KeyError, subprocess.CalledProcessError, subprocess.SubprocessError):
        pass
    return None

def append_to_portfolio(line):
    with open("portfolio.md", "a") as f:
        f.write(line + "\n")

def parse_signals_md():
    signals = []

    file_path = None
    if os.path.exists("Signals.md"):
        file_path = "Signals.md"
    elif os.path.exists("signals.md"):
        file_path = "signals.md"

    if not file_path:
        return signals

    with open(file_path, "r") as f:
        lines = f.readlines()
        in_table = False
        for line in lines:
            if "| Timestamp" in line or "| Date" in line:
                in_table = True
                continue
            if in_table and line.strip().startswith("|"):
                parts = [p.strip() for p in line.split("|")]
                if len(parts) >= 6 and parts[1] != "---":
                    try:
                        # Format: | Timestamp | Market | Symbol | Action | Confidence | Rationale |
                        signals.append({
                            "symbol": parts[3],
                            "market": parts[2],
                            "action": parts[4].lower(),
                            "confidence": float(parts[5].replace("%", "")) if "%" in parts[5] else 0.0,
                            "ref": f"{parts[2]}:{parts[3]}:{parts[4].lower()}:{parts[1]}"
                        })
                    except (ValueError, IndexError, TypeError, KeyError):
                        pass
    return signals

def main():
    pending_signals = parse_signals_md()

    for asset in SELECTED_CANDIDATES:
        symbol = asset["symbol"]
        provider = asset["provider"]
        market = asset["market"]

        print(f"\nEvaluating candidate: {symbol} ({provider})")

        data_file = f"data_{symbol}.json"
        analysis_file = f"analysis_{symbol}.json"

        # Fetch Market Data
        print(f"  Fetching market data...")
        res = run_cmd(f"cargo run -p thales-cli -- fetch-market-data --symbol {symbol} --provider {provider} --timeframe 1d")
        if not res:
            print(f"  Failed to fetch data for {symbol}")
            continue

        with open(data_file, "w") as f:
            json.dump(res, f)

        # Analyze Market
        print(f"  Analyzing market data...")
        res = run_cmd(f"cargo run -p thales-cli -- analyze-market --input {data_file} --no-report")
        if not res:
            print(f"  Failed to analyze market for {symbol}")
            if os.path.exists(data_file): os.remove(data_file)
            continue

        with open(analysis_file, "w") as f:
            json.dump(res, f)

        # Generate Signals across strategies
        print(f"  Generating signals across all active strategies...")
        all_intents = []
        conflicting = False
        directions = set()

        for strategy in STRATEGIES:
            intent_file = f"intent_{symbol}_{strategy}.json"
            cmd = f"cargo run -p thales-cli -- generate-signals --input {data_file} --strategy {strategy} --analysis {analysis_file}"
            res = run_cmd(cmd)

            if res and isinstance(res, list) and len(res) > 0:
                intent = res[0]
                if intent.get("signal_type") != "Hold":
                    all_intents.append((strategy, intent))
                    direction = "buy" if intent.get("signal_type") in ("Entry", "ScaleIn") else "sell"
                    directions.add(direction)
                    with open(intent_file, "w") as f:
                        json.dump(intent, f)

        # Add any pending signals from Signals.md for this asset
        asset_pending_signals = [s for s in pending_signals if s["symbol"] == symbol]
        for s in asset_pending_signals:
            directions.add(s["action"])

        now_str = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d %H:%M:%S")

        if len(directions) > 1:
            conflicting = True
            print(f"  Conflict detected for {symbol}: {directions}")
            rejection_reason = f"Conflict: Multiple strategies gave conflicting signals ({directions})"
            append_to_portfolio(f"| {now_str} | {symbol} | NO_REF | {rejection_reason} |")
        elif len(all_intents) > 0 and not conflicting:
            # Pick best intent (first if none, since we assume they agree on direction)
            # Find the intent with the highest confidence
            all_intents.sort(key=lambda x: x[1].get("confidence", 0.0), reverse=True)
            best_strategy = all_intents[0][0]
            best_intent = all_intents[0][1]

            intent_file = f"intent_{symbol}_{best_strategy}.json"

            print(f"  Valid signal found! Executing {best_intent.get('side')} for {symbol} using {best_strategy}")

            # Execute Intent
            exec_cmd = f"cargo run -p thales-cli -- execute-intent --provider {provider} --input {intent_file}"
            exec_res = subprocess.run(exec_cmd, shell=True, capture_output=True, text=True)

            try:
                exec_data = json.loads(exec_res.stdout)
                if exec_data.get("status") == "ok":
                    order = exec_data.get("data")
                    # Depending on API, response shape differs
                    action = best_intent.get("side", "unknown")
                    signal_type = best_intent.get("signal_type", "Entry")
                    action_fmt = f"{action} ({signal_type})"
                    qty = order.get("qty", best_intent.get("size_hint", "0"))
                    price = order.get("price", "Market")
                    sl = best_intent.get("stop_loss", "-")
                    if sl is None: sl = "-"
                    tp = best_intent.get("take_profit", "-")
                    if tp is None: tp = "-"

                    # Compute max risk
                    max_risk = "-"
                    if sl != "-" and price != "Market":
                        try:
                            max_risk = f"{abs(float(price) - float(sl)) * float(qty):.2f}"
                        except (ValueError, TypeError):
                            max_risk = "100" # fallback
                    else:
                        if sl != "-": max_risk = "100"

                    ref = best_intent.get("intent_id", f"NO_REF")
                    rationale = f"Strategy: {best_strategy} ({best_intent.get('confidence', 0.0)*100:.0f}%). {best_intent.get('rationale', '')}"
                    append_to_portfolio(f"| {now_str} | {market} | {symbol} | {action_fmt} | {qty} | {price} | {sl} | {tp} | {max_risk} | {ref} | {rationale} |")
                else:
                    err_msg = exec_data.get("errors", ["Execution Failed"])[0]
                    append_to_portfolio(f"| {now_str} | {symbol} | NO_REF | provider error: {err_msg} |")
                    print(f"  Execution failed: {err_msg}")
            except (json.JSONDecodeError, ValueError, TypeError, IndexError, KeyError, OSError) as e:
                append_to_portfolio(f"| {now_str} | {symbol} | NO_REF | Execution Failed: {str(e)} |")
                print(f"  Execution failed exception: {str(e)}")
        else:
            print(f"  No valid signals for {symbol}")
            append_to_portfolio(f"| {now_str} | {symbol} | NO_REF | No strategy signal generated. |")

        # Cleanup
        if os.path.exists(data_file): os.remove(data_file)
        if os.path.exists(analysis_file): os.remove(analysis_file)
        for strategy in STRATEGIES:
            fpath = f"intent_{symbol}_{strategy}.json"
            if os.path.exists(fpath):
                os.remove(fpath)

if __name__ == "__main__":
    main()
