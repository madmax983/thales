import subprocess
import json
import os
import datetime

from execute_cycle import verify_risk

CLI_PATH = "./target/release/thales-cli"
HISTORY_PATH = "history.json"

def run_command(args):
    cmd = [CLI_PATH] + args
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        return None
    try:
        # Find JSON envelope
        start = result.stdout.find("{")
        if start != -1:
            data = json.loads(result.stdout[start:])
            if data.get("status") == "ok":
                return data.get("data")
    except:
        pass
    return None

def format_signal(intent):
    direction = "long" if str(intent.get('side', '')).lower() == "buy" else "short"
    strength = intent.get('confidence', 0.0) * 100
    size = intent.get('size_hint', '0')
    sl = intent.get('stop_loss', 'None')
    tp = intent.get('take_profit', 'None')
    reason = intent.get('rationale', 'No reason provided.')
    signal_type = intent.get('signal_type', 'Entry')

    # Clean up Rust enum string if present (e.g. SignalType::Entry -> Entry)
    signal_type = signal_type.replace("SignalType::", "")

    output = f"- Symbol and direction (long/short): {intent['symbol']} ({direction})\n"
    output += f"- Signal type and strength (0-100%): {signal_type}, Strength: {strength:.1f}%\n"
    output += f"- Suggested size (quantity): {size}\n"
    output += f"- Stop loss and take profit levels: Stop Loss: {sl}, Take Profit: {tp}\n"
    output += f"- Clear reasoning (including historical context): {reason}\n"
    return output

def main():
    if not os.path.exists(CLI_PATH):
        print(f"Error: {CLI_PATH} not found. Please run 'cargo build --release' first.")
        return

    # 1. Scan market
    print("Scanning market for candidates...")
    symbols = run_command(["scan-market", "--provider", "paper"])
    if not symbols:
        print("No candidates found.")
        return

    print(f"Found candidates: {', '.join(symbols)}")

    # Use active strategies from strategies.md (defaulting to a few for demo)
    active_strategies = ["BollingerBands", "RsiMeanReversion", "Macd", "Supertrend", "DonchianBreakout", "StochasticOscillator"]

    all_intents = []

    for symbol in symbols[:3]: # Limit to top candidates
        print(f"\nEvaluating {symbol}...")
        # 2. Fetch Data
        data_file = f"{symbol}_data.json"
        bars = run_command(["fetch-market-data", "--provider", "paper", "--symbol", symbol, "--timeframe", "1h"])
        if not bars:
            print(f"Failed to fetch data for {symbol}.")
            continue
        with open(data_file, "w") as f:
            json.dump(bars, f)

        # 3. Market Analysis
        analysis_file = f"{symbol}_analysis.json"
        analysis = run_command(["analyze-market", "--input", data_file, "--no-report"])
        if not analysis:
            print(f"Failed to analyze {symbol}.")
            os.remove(data_file)
            continue
        with open(analysis_file, "w") as f:
            json.dump(analysis, f)

        # 4. Generate Signals (includes search history check, sizing, SL/TP)
        symbol_intents = []
        for strategy in active_strategies:
            args = ["generate-signals", "--input", data_file, "--strategy", strategy, "--analysis", analysis_file]
            if os.path.exists(HISTORY_PATH):
                args.extend(["--history", HISTORY_PATH])

            intents = run_command(args)
            if intents:
                for intent in intents:
                    # Size positions based on volatility
                    volatility_label = analysis.get("volatility", "").lower() if analysis else ""
                    size_hint_str = intent.get("size_hint", "0")
                    if size_hint_str != "max":
                        try:
                            size_hint_val = float(size_hint_str)
                            if "high" in volatility_label or "extreme" in volatility_label:
                                size_hint_val *= 0.5
                            elif "low" in volatility_label:
                                size_hint_val *= 1.5
                            intent["size_hint"] = f"{size_hint_val:.6f}"
                        except ValueError:
                            pass

                    # Filter: Only allow Entry signals that have a valid stop loss
                    is_entry = intent.get("signal_type") in ["Entry", "SignalType::Entry"]
                    if is_entry and (not intent.get("stop_loss") or intent.get("stop_loss") == "None"):
                        continue

                    # Filter: Do not chase moves - wait for pullbacks
                    # Check the 'sentiment' string from analysis
                    if is_entry and analysis:
                        sentiment = analysis.get("sentiment", "").lower()
                        side = str(intent.get('side', '')).lower()
                        if side == "buy" and "(overbought)" in sentiment:
                            print(f"Skipping long signal for {symbol}: Chasing move (Sentiment is Overbought)")
                            continue
                        elif side == "sell" and "(oversold)" in sentiment:
                            print(f"Skipping short signal for {symbol}: Chasing move (Sentiment is Oversold)")
                            continue

                    # All signals must go through Risk Agent before execution
                    risk_ok, risk_reason = verify_risk(intent)
                    if not risk_ok:
                        print(f"Skipping signal for {symbol} due to Risk Agent rejection: {risk_reason}")
                        continue

                    symbol_intents.append(intent)

        # Limit to 1-3 signals per symbol per day and resolve conflicts
        symbol_intents.sort(key=lambda x: x.get("confidence", 0.0), reverse=True)

        # Resolve conflicting directions (only keep the direction of the highest confidence signal)
        if symbol_intents:
            primary_direction = symbol_intents[0].get('side', 'buy')
            # Limit strictly to 3 signals per symbol per day
            filtered_intents = [intent for intent in symbol_intents if intent.get('side', 'buy') == primary_direction][:3]
            all_intents.extend(filtered_intents)

        # Cleanup
        if os.path.exists(data_file):
            os.remove(data_file)
        if os.path.exists(analysis_file):
            os.remove(analysis_file)

    if not all_intents:
        print("\nNo signals generated.")
        return

    # Sort by confidence
    all_intents.sort(key=lambda x: x.get("confidence", 0.0), reverse=True)

    print("\n=== Signal Generator Output ===\n")
    for intent in all_intents:
        print(format_signal(intent))
        print("------------------\n")

if __name__ == "__main__":
    main()
