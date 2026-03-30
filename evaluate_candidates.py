import json
import subprocess
import os
import datetime
from pathlib import Path

# Config
CANDIDATES_KRAKEN = ["PEPEUSD", "REKTUSD", "MOGUSD"]
CANDIDATES_ALPACA = ["SPY", "QQQ", "TQQQ"]
POSITIONS_KRAKEN = []
POSITIONS_ALPACA = ["SPY"]

ALL_ASSETS = [
    {"symbol": sym, "provider": "kraken", "market": "crypto"} for sym in CANDIDATES_KRAKEN + POSITIONS_KRAKEN
] + [
    {"symbol": sym, "provider": "alpaca", "market": "equities"} for sym in CANDIDATES_ALPACA + POSITIONS_ALPACA
]

# Ensure uniqueness
seen = set()
UNIQUE_ASSETS = []
for a in ALL_ASSETS:
    if a["symbol"] not in seen:
        seen.add(a["symbol"])
        UNIQUE_ASSETS.append(a)

STRATEGIES = [
    "BollingerBands", "EmaCrossover", "RsiMeanReversion", "Macd", "Supertrend",
    "DonchianBreakout", "ParabolicSar", "KeltnerChannelBreakout", "StochasticOscillator",
    "AdxMomentum", "IchimokuCloud", "CciMomentum", "LinearRegressionTrend", "ForceIndexTrend", "ObvTrendFollowing",
    "MoneyFlowIndex", "ConnorsRsiMeanReversion", "AwesomeOscillator", "WilliamsR",
    "VwmaCrossover", "VwapReversion", "VortexBreakout", "ZScoreMeanReversion",
    "ChaikinMoneyFlow", "ElderRay", "ChandelierExit", "ChoppinessIndexTrend", "AroonOscillator", "RocMomentum",
    "KamaCrossover", "VolumeOscillatorTrend", "VptTrendFollowing", "PpoRsiTrend", "TrixCrossover", "ChaikinOscillatorMomentum"
]

def run_cmd(cmd):
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    if result.returncode != 0:
        return None
    try:
        data = json.loads(result.stdout)
        if data.get("status") == "ok":
            return data.get("data")
    except:
        pass
    return None

def run_cmd_raw(cmd):
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    if result.returncode != 0:
        return None
    try:
        return json.loads(result.stdout)
    except:
        pass
    return None

def append_to_portfolio(line):
    with open("portfolio.md", "a") as f:
        f.write(line + "\n")

def parse_signals_md():
    signals = []
    if not os.path.exists("Signals.md"):
        return signals
    with open("Signals.md", "r") as f:
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
                        # Assuming format: | Timestamp | Market | Symbol | Action | Confidence | Rationale |
                        signals.append({
                            "symbol": parts[3],
                            "market": parts[2],
                            "action": parts[4].lower(),
                            "confidence": float(parts[5].replace("%", "")) if "%" in parts[5] else 0.0,
                            "ref": f"{parts[2]}:{parts[3]}:{parts[4].lower()}:{parts[1]}"
                        })
                    except:
                        pass
    return signals

def main():
    pending_signals = parse_signals_md()

    for asset in UNIQUE_ASSETS:
        symbol = asset["symbol"]
        provider = asset["provider"]
        market = asset["market"]

        print(f"Processing {symbol} ({provider})")

        data_file = f"data_{symbol}.json"
        analysis_file = f"analysis_{symbol}.json"

        # 1. Fetch Market Data
        res = run_cmd(f"cargo run -p thales-cli -- fetch-market-data --symbol {symbol} --provider {provider} --timeframe 1d")
        if not res:
            print(f"  Failed to fetch data for {symbol}")
            continue

        with open(data_file, "w") as f:
            json.dump(res, f)

        # 2. Analyze Market
        res = run_cmd(f"cargo run -p thales-cli -- analyze-market --input {data_file} --no-report")
        if not res:
            print(f"  Failed to analyze market for {symbol}")
            if os.path.exists(data_file): os.remove(data_file)
            continue

        with open(analysis_file, "w") as f:
            json.dump(res, f)

        # 3. Generate Signals across strategies
        all_intents = []
        conflicting = False
        directions = set()

        for strategy in STRATEGIES:
            intent_file = f"intent_{symbol}_{strategy}.json"
            # Use raw to get the actual array of intents or envelope
            cmd = f"cargo run -p thales-cli -- generate-signals --input {data_file} --strategy {strategy} --analysis {analysis_file}"
            res = run_cmd(cmd)

            if res and isinstance(res, list) and len(res) > 0:
                intent = res[0]
                if intent.get("signal_type") != "Hold":
                    all_intents.append((strategy, intent))
                    direction = "buy" if intent.get("signal_type") == "Entry" else "sell"
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
            rejection_reason = f"CONFLICT: Conflicting signals (Buy and Sell) detected for {symbol}. Trading halted for this asset."
            append_to_portfolio(f"| {now_str} | {symbol} | NO_REF | {rejection_reason} |")
        elif len(all_intents) > 0 and not conflicting:
            # Pick best intent (highest confidence, or first if none)
            best_intent = all_intents[0][1]
            best_strategy = all_intents[0][0]

            intent_file = f"intent_{symbol}_{best_strategy}.json"

            print(f"  Executing {best_intent.get('signal_type')} for {symbol} using {best_strategy}")

            # 4. Execute Intent
            # To execute, we need to pass the file path
            # Route both equities and crypto to Kraken ONLY for execution
            exec_provider = "kraken" if os.environ.get("SIMULATION") != "true" else "paper"
            exec_cmd = f"cargo run -p thales-cli -- execute-intent --provider {exec_provider} --input {intent_file}"
            exec_res = subprocess.run(exec_cmd, shell=True, capture_output=True, text=True)

            try:
                exec_data = json.loads(exec_res.stdout)
                if exec_data.get("status") == "ok":
                    order = exec_data.get("data")
                    action = "buy" if best_intent.get("signal_type") == "Entry" else "sell"
                    qty = order.get("qty", best_intent.get("size_hint", "0"))
                    price = order.get("price", "Market")
                    sl = best_intent.get("stop_loss", "-")
                    tp = best_intent.get("take_profit", "-")
                    ref = f"{market}:{symbol}:{action}:{int(datetime.datetime.now(datetime.timezone.utc).timestamp()*1000)}"
                    rationale = f"Strategy: {best_strategy}"
                    append_to_portfolio(f"| {now_str} | {market} | {symbol} | {action} | {qty} | {price} | {sl} | {tp} | - | {ref} | {rationale} |")
                else:
                    err_msg = exec_data.get("errors", ["Execution Failed"])[0]
                    append_to_portfolio(f"| {now_str} | {symbol} | NO_REF | provider error: {err_msg} |")
            except:
                append_to_portfolio(f"| {now_str} | {symbol} | NO_REF | Execution Failed |")
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
