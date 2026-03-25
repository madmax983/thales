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

def count_recent_signals(symbol, history_path):
    if not os.path.exists(history_path):
        return 0
    try:
        with open(history_path, "r") as f:
            history = json.load(f)
    except (json.JSONDecodeError, OSError):
        return 0

    now_ms = int(datetime.datetime.now(datetime.timezone.utc).timestamp() * 1000)
    day_ms = 24 * 60 * 60 * 1000
    count = 0
    for entry in history:
        intent = entry.get("intent", {})
        if intent.get("symbol") == symbol:
            analysis = entry.get("market_analysis", {})
            ts = analysis.get("timestamp_unix_ms", 0)
            if now_ms - ts <= day_ms:
                count += 1
    return count

def format_price(val):
    if val in [None, 'None', '-']:
        return 'None'
    try:
        formatted_val = f"{float(val):.4f}".rstrip('0').rstrip('.') if '.' in f"{float(val):.4f}" else f"{float(val):.4f}"
        return formatted_val if formatted_val else "0"
    except (ValueError, TypeError):
        return str(val)

def is_missing(val):
    if not val or str(val) in ["None", "-", "0", "0.0"]:
        return True
    try:
        if float(val) == 0.0:
            return True
    except (ValueError, TypeError):
        pass
    return False

def format_signal(intent):
    side = str(intent.get('side', '')).lower()
    if side == "buy":
        direction = "long"
    elif side == "sell":
        direction = "short"
    else:
        direction = side

    strength = intent.get('confidence', 0.0) * 100

    size = format_price(intent.get('size_hint', '0'))
    sl = format_price(intent.get('stop_loss', 'None'))
    tp = format_price(intent.get('take_profit', 'None'))
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

        recent_signals_count = count_recent_signals(symbol, HISTORY_PATH)
        if recent_signals_count >= 3:
            print(f"Skipping {symbol}: Already reached daily limit of 3 signals.")
            # Cleanup
            if os.path.exists(data_file):
                os.remove(data_file)
            if os.path.exists(analysis_file):
                os.remove(analysis_file)
            continue

        # 4. Generate Signals (includes RAG check, sizing, SL/TP)
        symbol_intents = []
        for strategy in active_strategies:
            args = ["generate-signals", "--input", data_file, "--strategy", strategy, "--analysis", analysis_file]
            if os.path.exists(HISTORY_PATH):
                args.extend(["--history", HISTORY_PATH])

            intents = run_command(args)
            if intents:
                for intent in intents:
                    # Filter: Never generate signals without proper analysis
                    if not analysis or len(analysis) == 0:
                        print(f"Skipping signal for {symbol}: No proper analysis available.")
                        continue

                    # Handle Signal Types: Entry, Exit, ScaleIn, ScaleOut
                    signal_type_raw = intent.get("signal_type", "")
                    is_entry = signal_type_raw in ["Entry", "SignalType::Entry"]
                    is_exit = signal_type_raw in ["Exit", "SignalType::Exit"]
                    is_scale_in = signal_type_raw in ["ScaleIn", "SignalType::ScaleIn"]
                    is_scale_out = signal_type_raw in ["ScaleOut", "SignalType::ScaleOut"]


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
                            formatted_sz = f"{size_hint_val:.4f}".rstrip('0').rstrip('.') if '.' in f"{size_hint_val:.4f}" else f"{size_hint_val:.4f}"
                            intent["size_hint"] = formatted_sz if formatted_sz else "0"
                        except (ValueError, TypeError):
                            pass

                    if is_exit or is_scale_out:
                        intent["size_hint"] = "max" if is_exit else intent.get("size_hint", "max")
                        if is_scale_out:
                            try:
                                current_sz = float(intent.get("size_hint", "0"))
                                intent["size_hint"] = format_price(current_sz * 0.5)
                            except (ValueError, TypeError):
                                pass
                        intent.pop("stop_loss", None)
                        intent.pop("take_profit", None)
                    elif is_entry or is_scale_in:
                        if is_scale_in:
                            try:
                                current_sz = float(intent.get("size_hint", "0"))
                                intent["size_hint"] = format_price(current_sz * 0.5)
                            except (ValueError, TypeError):
                                pass

                        sl_val = intent.get("stop_loss")
                        tp_val = intent.get("take_profit")
                        missing_sl = is_missing(sl_val)
                        missing_tp = is_missing(tp_val)

                        if missing_sl or missing_tp:
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

                                        if side in ["buy", "long"]:
                                            if missing_sl:
                                                intent["stop_loss"] = str(last_close * (1.0 - sl_pct))
                                            if missing_tp:
                                                try:
                                                    sl_val_check = float(intent["stop_loss"])
                                                    sl_dist = last_close - sl_val_check
                                                    if sl_dist > 0:
                                                        intent["take_profit"] = str(last_close + (sl_dist * 2.0))
                                                    else:
                                                        intent["take_profit"] = str(last_close * (1.0 + tp_pct))
                                                except (ValueError, TypeError, KeyError):
                                                    intent["take_profit"] = str(last_close * (1.0 + tp_pct))
                                        elif side in ["sell", "short"]:
                                            if missing_sl:
                                                intent["stop_loss"] = str(last_close * (1.0 + sl_pct))
                                            if missing_tp:
                                                try:
                                                    sl_val_check = float(intent["stop_loss"])
                                                    sl_dist = sl_val_check - last_close
                                                    if sl_dist > 0:
                                                        intent["take_profit"] = str(last_close - (sl_dist * 2.0))
                                                    else:
                                                        intent["take_profit"] = str(last_close * (1.0 - tp_pct))
                                                except (ValueError, TypeError, KeyError):
                                                    intent["take_profit"] = str(last_close * (1.0 - tp_pct))
                            except (FileNotFoundError, json.JSONDecodeError, KeyError, ValueError, IndexError) as e:
                                print(f"Warning: Failed to compute fallback SL/TP for {symbol}: {e}")

                    # Filter: Only allow Entry/ScaleIn signals that have a valid stop loss
                    sl_val_check = intent.get("stop_loss")
                    if (is_entry or is_scale_in) and is_missing(sl_val_check):
                        print(f"Skipping signal for {symbol}: Missing mandatory stop loss.")
                        continue

                    # Filter: Do not chase moves - wait for pullbacks
                    if is_entry:
                        side = str(intent.get('side', '')).lower()
                        sentiment = analysis.get("sentiment", "").lower()

                        # We specifically look for (overbought) / (oversold) in the sentiment
                        # To avoid false positives on rationale like "not overbought", we check sentiment primarily.
                        if side in ["buy", "long"] and "overbought" in sentiment:
                            print(f"Skipping long signal for {symbol}: Chasing move (Sentiment is Overbought)")
                            continue
                        elif side in ["sell", "short"] and "oversold" in sentiment:
                            print(f"Skipping short signal for {symbol}: Chasing move (Sentiment is Oversold)")
                            continue

                    # Filter: Check historical trades before generating new signals
                    rationale = intent.get("rationale") or ""
                    if "No similar past trades found" in rationale:
                        print(f"Note: No similar past trades found for {symbol}.")
                        # We don't skip the signal, we just note it as it might be a valid new setup

                    # Format numerical values safely to 4 decimal places
                    for field in ["size_hint", "stop_loss", "take_profit"]:
                        if field in intent:
                            intent[field] = format_price(intent[field])

                    # Route all signals through the Risk Agent first
                    # We get the last close price for Risk Agent if possible
                    last_close = None
                    try:
                        with open(data_file, "r") as f:
                            b_data = json.load(f)
                            if "bars" in b_data and len(b_data["bars"]) > 0:
                                last_close = float(b_data["bars"][-1]["close"])
                    except (FileNotFoundError, json.JSONDecodeError, KeyError, ValueError, IndexError):
                        pass

                    risk_ok, risk_reason = verify_risk(intent, current_price=last_close) if last_close is not None else verify_risk(intent)
                    if not risk_ok:
                        print(f"Skipping signal for {symbol} due to Risk Agent rejection: {risk_reason}")
                        continue

                    # Map buy/sell to long/short
                    side = str(intent.get('side', '')).lower()
                    if side == "buy":
                        intent['side'] = "long"
                    elif side == "sell":
                        intent['side'] = "short"

                    symbol_intents.append(intent)

        # Limit to 1-3 signals per symbol per day and resolve conflicts
        symbol_intents.sort(key=lambda x: x.get("confidence", 0.0), reverse=True)

        # Resolve conflicting directions (only keep the direction of the highest confidence signal)
        if symbol_intents:
            primary_direction = symbol_intents[0].get('side', 'long')
            filtered_intents = [intent for intent in symbol_intents if intent.get('side', 'long') == primary_direction]

            # Limit strictly to 1-3 signals per symbol per day
            # We already skipped processing if history had >= 3.
            # Now we just need to ensure the NEW signals we add don't exceed the daily limit of 3
            # OR a minimum of 1 if history has 0 but we have valid signals.
            allowance = max(0, 3 - recent_signals_count)

            # If we don't have enough to fill the allowance, take what we have
            # But the requirement is "Limit to 1-3 signals per symbol per day".
            # The slicing `[:allowance]` already caps it.
            filtered_intents = filtered_intents[:allowance]
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
