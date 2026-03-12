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
    except (json.JSONDecodeError, ValueError, KeyError):
        pass
    return None

def format_signal(intent):
    direction = "long" if str(intent.get('side', '')).lower() == "buy" else "short"
    direction = direction.lower()
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

        # 4. Generate Signals (includes RAG check, sizing, SL/TP)
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
                        except (ValueError, TypeError):
                            pass

                    # Handle Signal Types: Entry, Exit, ScaleIn, ScaleOut
                    signal_type_raw = intent.get("signal_type", "")
                    is_entry = signal_type_raw in ["Entry", "SignalType::Entry"]
                    is_exit = signal_type_raw in ["Exit", "SignalType::Exit"]
                    is_scale_in = signal_type_raw in ["ScaleIn", "SignalType::ScaleIn"]
                    is_scale_out = signal_type_raw in ["ScaleOut", "SignalType::ScaleOut"]

                    if is_exit:
                        intent["size_hint"] = "max"
                        intent.pop("stop_loss", None)
                        intent.pop("take_profit", None)
                    elif is_scale_out:
                        try:
                            current_sz = float(intent.get("size_hint", "0"))
                            intent["size_hint"] = f"{(current_sz * 0.5):.6f}"
                        except (ValueError, TypeError):
                            pass
                        intent.pop("stop_loss", None)
                        intent.pop("take_profit", None)
                    elif is_entry or is_scale_in:
                        if is_scale_in:
                            try:
                                current_sz = float(intent.get("size_hint", "0"))
                                intent["size_hint"] = f"{(current_sz * 0.5):.6f}"
                            except (ValueError, TypeError):
                                pass

                        if not intent.get("stop_loss") or intent.get("stop_loss") == "None":
                            try:
                                with open(data_file, "r") as f:
                                    b_data = json.load(f)
                                    if "bars" in b_data and len(b_data["bars"]) > 0:
                                        last_close = float(b_data["bars"][-1]["close"])
                                        side = str(intent.get('side', '')).lower()

                                        # Calculate stop loss distance based on volatility
                                        volatility_label = analysis.get("volatility", "").lower() if analysis else ""
                                        sl_pct = 0.05
                                        if "high" in volatility_label or "extreme" in volatility_label:
                                            sl_pct = 0.10
                                        elif "low" in volatility_label:
                                            sl_pct = 0.02

                                        tp_pct = sl_pct * 2.0

                                        if side == "buy":
                                            intent["stop_loss"] = last_close * (1.0 - sl_pct)
                                            if not intent.get("take_profit") or intent.get("take_profit") == "None":
                                                intent["take_profit"] = last_close * (1.0 + tp_pct)
                                        elif side == "sell":
                                            intent["stop_loss"] = last_close * (1.0 + sl_pct)
                                            if not intent.get("take_profit") or intent.get("take_profit") == "None":
                                                intent["take_profit"] = last_close * (1.0 - tp_pct)
                            except (FileNotFoundError, json.JSONDecodeError, KeyError, ValueError, IndexError) as e:
                                print(f"Warning: Failed to compute fallback stop loss for {symbol}: {e}")

                    # Filter: Never generate signals without proper analysis
                    if not analysis:
                        print(f"Skipping signal for {symbol}: No proper analysis available.")
                        continue

                    # Filter: Only allow Entry/ScaleIn signals that have a valid stop loss and take profit
                    if (is_entry or is_scale_in) and (
                        not intent.get("stop_loss") or intent.get("stop_loss") == "None" or
                        not intent.get("take_profit") or intent.get("take_profit") == "None"
                    ):
                        print(f"Skipping signal for {symbol}: Missing mandatory stop loss or take profit.")
                        continue

                    # Filter: Check historical trades before generating new signals
                    rationale = intent.get("rationale") or ""
                    # Note: We do not skip if "No similar past trades found" is present,
                    # because a lack of history is a valid historical context for a new signal.

                    # Filter: Do not chase moves - wait for pullbacks
                    # Check the 'sentiment' string from analysis and rationale
                    if is_entry:
                        side = str(intent.get('side', '')).lower()
                        sentiment = analysis.get("sentiment", "").lower()

                        # We specifically look for (Overbought) / (Oversold) in the sentiment
                        # To avoid false positives on rationale like "not overbought", we check sentiment primarily.
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
