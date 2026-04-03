import subprocess
import json
import os
import sys
import datetime
import re

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

def apply_volatility_sizing(size_hint_val, volatility_label):
    if "high" in volatility_label or "extreme" in volatility_label:
        return size_hint_val * 0.5
    elif "low" in volatility_label:
        return size_hint_val * 1.5
    return size_hint_val

def format_size(val):
    if val in [None, 'None', '-', "", "max"]:
        return str(val)
    try:
        val_float = float(val)
        if val_float == 0.0:
            return "0"
        formatted_val = f"{val_float:.8f}".rstrip('0').rstrip('.')
        return formatted_val if formatted_val else "0"
    except (ValueError, TypeError):
        return str(val)

def format_price(val):
    if val in [None, 'None', '-', ""]:
        return 'None'
    try:
        val_float = float(val)
        if val_float == 0.0:
            return "0"
        formatted_val = f"{val_float:.4f}".rstrip('0').rstrip('.')
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

def calculate_fallback_sl_tp(last_close, side, volatility_label, missing_sl, missing_tp, intent_sl):
    sl_pct = 0.05
    if "high" in volatility_label or "extreme" in volatility_label:
        sl_pct = 0.10
    elif "low" in volatility_label:
        sl_pct = 0.02

    tp_pct = sl_pct * 2.0

    new_sl = None
    new_tp = None

    if side in ["buy", "long"]:
        if missing_sl:
            new_sl = format_price(last_close * (1.0 - sl_pct))
        if missing_tp:
            try:
                sl_val_check = float(format_price(intent_sl)) if not missing_sl else float(new_sl)
                sl_dist = last_close - sl_val_check
                if sl_dist > 0:
                    new_tp = format_price(last_close + (sl_dist * 2.0))
                else:
                    new_tp = format_price(last_close * (1.0 + tp_pct))
            except (ValueError, TypeError, KeyError):
                new_tp = format_price(last_close * (1.0 + tp_pct))
    elif side in ["sell", "short"]:
        if missing_sl:
            new_sl = format_price(last_close * (1.0 + sl_pct))
        if missing_tp:
            try:
                sl_val_check = float(format_price(intent_sl)) if not missing_sl else float(new_sl)
                sl_dist = sl_val_check - last_close
                if sl_dist > 0:
                    new_tp = format_price(last_close - (sl_dist * 2.0))
                else:
                    new_tp = format_price(last_close * (1.0 - tp_pct))
            except (ValueError, TypeError, KeyError):
                new_tp = format_price(last_close * (1.0 - tp_pct))

    return new_sl, new_tp

def truncate_float(match):
    val_float = float(match.group(0))
    if val_float == 0.0:
        return "0"
    formatted_val = f"{val_float:.4f}".rstrip('0').rstrip('.')
    return formatted_val if formatted_val else "0"

def format_signal(intent):
    # Ensure logic explicitly supports both 'buy'/'sell' and 'long'/'short' string variants
    side_raw = intent.get('side', '')
    side = str(side_raw).lower() if side_raw else ''
    if side in ["buy", "long"]:
        direction = "long"
    elif side in ["sell", "short"]:
        direction = "short"
    else:
        direction = side

    strength = intent.get('confidence', 0.0) * 100

    size = format_size(intent.get('size_hint', '0'))

    sl_val = intent.get('stop_loss')
    tp_val = intent.get('take_profit')

    # Explicit missing variable checks
    sl = format_price(sl_val) if not is_missing(sl_val) else 'None'
    tp = format_price(tp_val) if not is_missing(tp_val) else 'None'

    reason = intent.get('rationale', 'No reason provided.')
    reason = re.sub(r'\d+\.\d{5,}', truncate_float, reason).strip()
    signal_type = intent.get('signal_type', 'Entry')

    # Clean up Rust enum string if present (e.g. SignalType::Entry -> Entry)
    signal_type = signal_type.replace("SignalType::", "")

    # Format output perfectly to match the user's requested output template
    output = f"- Symbol and direction (long/short): {intent.get('symbol')} ({direction})\n"
    output += f"- Signal type and strength (0-100%): {signal_type} ({strength:.1f}%)\n"
    output += f"- Suggested size (quantity): {size}\n"
    output += f"- Stop loss and take profit levels: SL: {sl}, TP: {tp}\n"
    output += f"- Clear reasoning (including historical context): {reason}"
    return output

def main():
    if not os.path.exists(CLI_PATH):
        print(f"Error: {CLI_PATH} not found. Please run 'cargo build --release' first.", file=sys.stderr)
        return

    # 1. Scan market
    print("Scanning market for candidates...", file=sys.stderr)
    symbols = run_command(["scan-market", "--provider", "paper"])
    if not symbols:
        print("No candidates found.", file=sys.stderr)
        return

    print(f"Found candidates: {', '.join(symbols)}", file=sys.stderr)

    # Use active strategies from strategies.md (defaulting to a few for demo)
    active_strategies = ["BollingerBands", "RsiMeanReversion", "Macd", "Supertrend", "DonchianBreakout", "StochasticOscillator"]

    all_intents = []

    for symbol in symbols[:3]: # Limit to top candidates
        print(f"\nEvaluating {symbol}...", file=sys.stderr)
        # 2. Fetch Data
        data_file = f"{symbol}_data.json"
        bars = run_command(["fetch-market-data", "--provider", "paper", "--symbol", symbol, "--timeframe", "1h"])
        if not bars:
            print(f"Failed to fetch data for {symbol}.", file=sys.stderr)
            continue
        with open(data_file, "w") as f:
            json.dump(bars, f)

        # Parse last close price for SL/TP and Risk Agent
        last_close_price = None
        try:
            with open(data_file, "r") as f:
                b_data = json.load(f)
                if "bars" in b_data and len(b_data["bars"]) > 0:
                    last_close_price = float(b_data["bars"][-1]["close"])
        except (FileNotFoundError, json.JSONDecodeError, KeyError, ValueError, IndexError):
            pass

        # 3. Market Analysis
        analysis_file = f"{symbol}_analysis.json"
        analysis = run_command(["analyze-market", "--input", data_file, "--no-report"])
        if not analysis:
            print(f"Failed to analyze {symbol}.", file=sys.stderr)
            os.remove(data_file)
            continue
        with open(analysis_file, "w") as f:
            json.dump(analysis, f)

        recent_signals_count = count_recent_signals(symbol, HISTORY_PATH)
        if recent_signals_count >= 3:
            print(f"Skipping {symbol}: Already reached daily limit of 3 signals.", file=sys.stderr)
            # Cleanup
            if os.path.exists(data_file):
                os.remove(data_file)
            if os.path.exists(analysis_file):
                os.remove(analysis_file)
            continue

        # 4. Generate Signals (includes RAG check, sizing, SL/TP)

        # Filter: Never generate signals without proper analysis (enforce at very top)
        if not analysis or len(analysis) == 0:
            print(f"Skipping signal generation for {symbol}: No proper analysis available.", file=sys.stderr)
            continue

        # Pre-calculate RAG (history) checking natively in Python to inject into rationale
        similar_trades_str = "No similar past trades found."
        if os.path.exists(HISTORY_PATH):
            try:
                with open(HISTORY_PATH, "r") as f:
                    history = json.load(f)
                    past_trades = []
                    for entry in history:
                        intent = entry.get("intent", {})
                        if intent.get("symbol") == symbol:
                            # Basic summary of past trade
                            outcome = entry.get("outcome")
                            status = "win" if outcome is not None and outcome > 0 else "loss" if outcome is not None else "unknown"
                            side = intent.get("side", "unknown")
                            past_trades.append(f"{side} ({status})")
                    if past_trades:
                        similar_trades_str = f"Found {len(past_trades)} past trades for {symbol}: {', '.join(past_trades[-3:])}."
            except (json.JSONDecodeError, OSError):
                pass

        symbol_intents = []
        limit_reached = False
        for strategy in active_strategies:
            if limit_reached:
                break

            args = ["generate-signals", "--input", data_file, "--strategy", strategy, "--analysis", analysis_file]
            if os.path.exists(HISTORY_PATH):
                args.extend(["--history", HISTORY_PATH])

            intents = run_command(args)
            if intents:
                for intent in intents:
                    # Format numerical values safely before checks
                    for field in ["stop_loss", "take_profit"]:
                        if field in intent:
                            intent[field] = format_price(intent[field])
                    if "size_hint" in intent:
                        intent["size_hint"] = format_size(intent["size_hint"])

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
                            adjusted_size = apply_volatility_sizing(size_hint_val, volatility_label)
                            intent["size_hint"] = format_size(adjusted_size)
                        except (ValueError, TypeError):
                            pass

                    if is_exit or is_scale_out:
                        intent["size_hint"] = "max" if is_exit else intent.get("size_hint", "max")
                        if is_scale_out:
                            try:
                                current_sz = float(intent.get("size_hint", "0"))
                                intent["size_hint"] = format_size(current_sz * 0.5)
                            except (ValueError, TypeError):
                                pass
                        intent.pop("stop_loss", None)
                        intent.pop("take_profit", None)
                    elif is_entry or is_scale_in:
                        if is_scale_in:
                            try:
                                current_sz = float(intent.get("size_hint", "0"))
                                intent["size_hint"] = format_size(current_sz * 0.5)
                            except (ValueError, TypeError):
                                pass

                        sl_val = intent.get("stop_loss")
                        tp_val = intent.get("take_profit")
                        missing_sl = is_missing(sl_val)
                        missing_tp = is_missing(tp_val)

                        if missing_sl or missing_tp:
                            if last_close_price is not None:
                                side_raw = intent.get('side', '')
                                side = str(side_raw).lower() if side_raw else ''
                                vol_label = analysis.get("volatility", "").lower() if analysis else ""
                                new_sl, new_tp = calculate_fallback_sl_tp(last_close_price, side, vol_label, missing_sl, missing_tp, sl_val)

                                if new_sl is not None:
                                    intent["stop_loss"] = format_price(new_sl)
                                if new_tp is not None:
                                    intent["take_profit"] = format_price(new_tp)
                            else:
                                print(f"Warning: Failed to compute fallback SL/TP for {symbol}: No last close price available.", file=sys.stderr)

                    # Re-format numerical values safely after fallback calculations
                    for field in ["stop_loss", "take_profit"]:
                        if field in intent:
                            intent[field] = format_price(intent[field])
                    if "size_hint" in intent:
                        intent["size_hint"] = format_size(intent["size_hint"])

                    # Filter: Only allow Entry/ScaleIn signals that have a valid stop loss
                    sl_val_check = intent.get("stop_loss")
                    if (is_entry or is_scale_in) and is_missing(sl_val_check):
                        print(f"Skipping signal for {symbol}: Missing mandatory stop loss.", file=sys.stderr)
                        continue

                    # Filter: Do not chase moves - wait for pullbacks
                    if is_entry:
                        side_raw = intent.get('side', '')
                        side = str(side_raw).lower() if side_raw else ''
                        sentiment = analysis.get("sentiment", "").lower()

                        # We specifically look for (overbought) / (oversold) in the sentiment
                        # To avoid false positives on rationale like "not overbought", we check sentiment primarily.
                        if side in ["buy", "long"] and "overbought" in sentiment:
                            print(f"Skipping long signal for {symbol}: Chasing move (Sentiment is Overbought)", file=sys.stderr)
                            continue
                        elif side in ["sell", "short"] and "oversold" in sentiment:
                            print(f"Skipping short signal for {symbol}: Chasing move (Sentiment is Oversold)", file=sys.stderr)
                            continue

                    # Filter: Check historical trades before generating new signals
                    rationale = intent.get("rationale") or ""

                    # Ensure the natively-calculated similar trades string is injected correctly
                    if "No similar past trades found" not in rationale and "past trades for" not in rationale:
                        rationale = f"{rationale} {similar_trades_str}"

                    intent["rationale"] = rationale

                    if "No similar past trades found" in rationale:
                        print(f"Note: No similar past trades found for {symbol}.", file=sys.stderr)
                        # We don't skip the signal, we just note it as it might be a valid new setup

                    # Map buy/sell to long/short
                    side_raw = intent.get('side', '')
                    side = str(side_raw).lower() if side_raw else ''
                    if side == "buy":
                        intent['side'] = "long"
                    elif side == "sell":
                        intent['side'] = "short"

                    symbol_intents.append(intent)

                    if recent_signals_count + len(symbol_intents) >= 3:
                        limit_reached = True
                        break

        # Filter: Verify Risk Agent check at the bottom after fallbacks and formatting
        validated_symbol_intents = []
        for intent in symbol_intents:
            risk_ok, risk_reason = verify_risk(intent, current_price=last_close_price) if last_close_price is not None else verify_risk(intent)
            if not risk_ok:
                print(f"Skipping signal for {symbol} due to Risk Agent rejection: {risk_reason}", file=sys.stderr)
                continue
            validated_symbol_intents.append(intent)

        symbol_intents = validated_symbol_intents

        # Limit to 1-3 signals per symbol per day and resolve conflicts
        symbol_intents.sort(key=lambda x: x.get("confidence", 0.0), reverse=True)

        # Resolve conflicting directions (only keep the direction of the highest confidence signal)
        if symbol_intents:
            primary_direction = symbol_intents[0].get('side', 'long')

            # Filter out duplicate intents with same parameters (e.g. from different strategies matching)
            seen_params = set()
            unique_intents = []
            for intent in symbol_intents:
                if intent.get('side', 'long') == primary_direction:
                    # Create a signature of the intent based on critical fields to filter duplicates
                    sig = (
                        intent.get("symbol"),
                        intent.get("side"),
                        intent.get("size_hint"),
                        intent.get("stop_loss"),
                        intent.get("take_profit"),
                        intent.get("signal_type")
                    )
                    if sig not in seen_params:
                        seen_params.add(sig)
                        unique_intents.append(intent)

            filtered_intents = unique_intents

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
        print("\nNo signals generated.", file=sys.stderr)
        return

    # Sort by confidence
    all_intents.sort(key=lambda x: x.get("confidence", 0.0), reverse=True)

    print("\n=== Signal Generator Output ===\n", file=sys.stderr)
    for i, intent in enumerate(all_intents):
        # Programmatically fulfill persona rule: log out structured signal
        print(format_signal(intent), file=sys.stdout)
        if i < len(all_intents) - 1:
            print("------------------\n")
        else:
            print("------------------")

if __name__ == "__main__":
    main()
# Signal Generator Agent modifications applied
