import json
import subprocess
import os
import sys
import time
from datetime import datetime

# Configuration
CLI_PATH = "./target/release/thales-cli"
PORTFOLIO_PATH = "portfolio.md"

def run_command(args):
    """Runs a thales-cli command and returns the parsed JSON data."""
    cmd = [CLI_PATH] + args
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=False)
        stdout = (result.stdout or "").strip()

        envelope = None
        if stdout:
            json_start = stdout.find("{")
            if json_start != -1:
                try:
                    envelope = json.loads(stdout[json_start:])
                except json.JSONDecodeError:
                    envelope = None

        if envelope and envelope.get("status") == "ok":
            return envelope.get("data")

        if result.returncode != 0:
            print(f"Command failed: {cmd}\nReason: {result.stderr or stdout}")
            return None

        return None
    except Exception as e:
        print(f"Exception running command {cmd}: {e}")
        return None

def manage_orders(provider):
    """
    FILL MANAGEMENT: Track order status and fills.
    Cancel stale orders (>5 min unfilled limits).
    Monitor for partial fills and adjust.
    """
    print(f"Checking open orders on {provider}...")
    orders = run_command(["get-open-orders", "--provider", provider])
    if not orders:
        return

    now = int(datetime.now().timestamp() * 1000)
    for order in orders:
        submitted_at = order.get("submitted_at_unix_ms", 0)
        age_ms = now - submitted_at
        age_s = age_ms / 1000.0

        filled_qty = float(order.get("filled_qty", 0.0))
        qty = float(order.get("qty", 0.0))

        if filled_qty > 0 and filled_qty < qty:
            print(f"Partial fill detected: {filled_qty}/{qty} for {order['symbol']} ({provider})")

            # Adjust partial fills (cancel and replace with market order for remainder)
            if age_ms > 300000:
                print(f"Cancelling stale partial order {order['id']}")
                run_command(["cancel-order", "--provider", provider, "--id", order['id']])
            else:
                print(f"Adjusting remaining quantity: {qty - filled_qty}")
                run_command(["cancel-order", "--provider", provider, "--id", order['id']])

                remaining = f"{qty - filled_qty:.8f}".rstrip("0").rstrip(".")
                side = order.get("side", "buy")
                symbol = order.get("symbol")

                adjustment_intent = {
                    "provider": provider,
                    "market": "unknown",
                    "symbol": symbol,
                    "side": side,
                    "size_hint": remaining,
                    "confidence": 1.0,
                    "rationale": f"Adjusting partial fill for order {order['id']}",
                    "intent_id": f"ADJUST-{order['id']}",
                    "order_type": "market",
                    "execution_algo": "Market"
                }

                temp_intent_file = f"temp_intent_adjust_{symbol}.json"
                with open(temp_intent_file, "w") as f:
                    json.dump(adjustment_intent, f)

                print(f"Submitting market order for remaining {remaining} {symbol}")
                run_command(["execute-intent", "--provider", provider, "--input", temp_intent_file])

                if os.path.exists(temp_intent_file):
                    os.remove(temp_intent_file)

        # Cancel stale orders (>5 min unfilled limits)
        elif age_ms > 300000:
            print(f"Cancelling stale order {order['id']} ({order['symbol']}) - Age: {age_s:.0f}s")
            run_command(["cancel-order", "--provider", provider, "--id", order['id']])

def refine_intent(intent, current_price=None):
    """
    ALGO SELECTION: Choose execution algorithm (market, limit, TWAP, VWAP)
    ORDER ROUTING: Select appropriate broker and order type
    """
    confidence = intent.get("confidence", 0.0)
    size_hint = intent.get("size_hint", "0")

    algo = "Limit"
    order_type = "limit"

    is_large = False
    is_very_large = False
    try:
        if size_hint != "max":
            size = float(size_hint)
            if size > 10000.0:
                is_very_large = True
            elif size > 1000.0:
                is_large = True
    except (ValueError, TypeError):
        pass

    if is_very_large:
        algo = "VWAP"
        order_type = "limit"
        print("Selected VWAP algorithm to minimize market impact for very large order.")
    elif is_large:
        algo = "TWAP"
        order_type = "limit"
        print("Selected TWAP algorithm for large order.")
    elif confidence >= 0.8 or size_hint == "max":
        algo = "Market"
        order_type = "market"
    else:
        algo = "Limit"
        order_type = "limit"

    if intent.get("stop_price"):
        if intent.get("limit_price"):
            order_type = "stop-limit"
            algo = "Limit"
        else:
            order_type = "stop"
            algo = "Market"

    intent["execution_algo"] = algo
    intent["order_type"] = order_type

    if (order_type == "limit" or order_type == "stop-limit") and not intent.get("limit_price") and current_price:
        intent["limit_price"] = current_price

    return intent

def monitor_execution(provider, order_id, expected_price):
    """
    SLIPPAGE CONTROL: Monitor and minimize execution slippage.
    """
    print(f"Monitoring execution for order {order_id}...")

    for _ in range(15):
        time.sleep(2)
        order = run_command(["get-order", "--provider", provider, "--id", order_id])
        if not order:
            continue

        status = order.get("status")
        if status == "filled":
            avg_price = order.get("average_fill_price")
            if avg_price:
                slippage = 0.0
                if expected_price and expected_price > 0:
                    side = order.get("side", "").lower()
                    if side == "buy":
                        slippage = (avg_price - expected_price) / expected_price * 100.0
                    else:
                        slippage = (expected_price - avg_price) / expected_price * 100.0

                print(f"Order filled at {avg_price} (Expected: {expected_price}). Slippage: {slippage:.4f}%")
                return avg_price, slippage
            else:
                return None, None
        elif status == "canceled":
            print("Order canceled.")
            return None, None

    print("Monitoring timed out (Order likely still open).")
    return None, None

def log_trade(intent, result, slippage=None):
    """
    REPORTING: Report execution results back to other agents.
    Log slippage for analysis.
    """
    date_str = datetime.fromtimestamp(result.get("submitted_at_unix_ms", int(datetime.now().timestamp() * 1000)) / 1000).strftime("%Y-%m-%d %H:%M:%S")
    asset_class = intent.get("market", "-")
    symbol = intent["symbol"]
    action = intent["side"]
    signal_type_raw = intent.get("signal_type", "") or ""

    if "SignalType::" in signal_type_raw:
        signal_type_clean = signal_type_raw.replace("SignalType::", "")
    else:
        signal_type_clean = signal_type_raw

    if signal_type_clean:
        action = f"{action} ({signal_type_clean})"

    size = intent["size_hint"]
    price = str(intent.get("limit_price", "Market"))
    sl = str(intent.get("stop_loss", "-"))
    tp = str(intent.get("take_profit", "-"))

    max_risk = "-"
    if sl != "-" and price != "Market":
        try:
             entry = float(price)
             stop = float(sl)
             qty = float(size) if size != "max" else 0.0
             if qty > 0:
                 max_risk = f"{abs(entry - stop) * qty:.2f}"
        except (ValueError, TypeError):
             pass

    signal_ref = intent.get("intent_id", "MANUAL")
    rationale = intent.get("rationale", "Manual Execution")

    if slippage is not None:
        rationale += f" [Slippage: {slippage:.4f}%]"

    header = "| Date/Time | Asset Class | Symbol/Contract | Action | Size/Qty | Entry Price | SL | TP | Max Risk | Signal Ref | Rationale |"
    row = f"| {date_str} | {asset_class} | {symbol} | {action} | {size} | {price} | {sl} | {tp} | {max_risk} | {signal_ref} | {rationale} |"

    print("\n=== EXECUTION REPORT ===")
    print(header)
    print(row)
    print("========================\n")

    # Append to portfolio.md
    if not os.path.exists(PORTFOLIO_PATH):
        with open(PORTFOLIO_PATH, "w") as f:
            f.write("## Executed Trades\n\n" + header + "\n" + "|---" * 11 + "|\n")

    with open(PORTFOLIO_PATH, "a") as f:
        f.write(row + "\n")

def execute_agent(intent_file):
    if not os.path.exists(CLI_PATH):
        print(f"Error: {CLI_PATH} not found. Please build the project first.")
        return

    if not os.path.exists(intent_file):
        print(f"Error: Intent file {intent_file} not found.")
        return

    print("=== Execution Agent Persona ===")

    with open(intent_file, "r") as f:
        loaded_json = json.load(f)

    # Check if this is a standard CLI JSON envelope or a bare intent list/object
    intents = []
    if isinstance(loaded_json, dict) and "data" in loaded_json:
        data = loaded_json["data"]
        if isinstance(data, list):
            intents = data
        else:
            intents = [data]
    elif isinstance(loaded_json, list):
        intents = loaded_json
    else:
        intents = [loaded_json]

    if not intents:
        print("Error: No intents found in the provided file.")
        return

    # For now, process the first intent, or loop through them
    for intent in intents:
        # ORDER ROUTING: Select appropriate broker and order type
        if os.environ.get("SIMULATION") == "true":
            intent["provider"] = "paper"
        else:
            intent["provider"] = "kraken"

        provider = intent.get("provider", "paper")

        # 1. Manage Orders (Stale & Partial Fills)
        manage_orders(provider)

        # 2. Get Current Price
        print(f"Fetching latest price for {intent['symbol']} on {provider}...")
        data = run_command(["fetch-market-data", "--provider", provider, "--symbol", intent['symbol'], "--timeframe", "1m"])
        current_price = None
        if isinstance(data, dict) and "bars" in data and len(data["bars"]) > 0:
            bars = data["bars"]
            bars.sort(key=lambda x: x.get("timestamp_unix_ms", 0))
            current_price = bars[-1].get("close")

        # 3. Refine Intent (Algo Selection & Order Type)
        intent = refine_intent(intent, current_price)
        print(f"Order Type: {intent['order_type'].upper()}")
        if intent.get("execution_algo"):
            print(f"Algorithm: {intent['execution_algo']}")

        # 4. Always set stop losses when available
        sl_val = intent.get("stop_loss")
        if not sl_val or str(sl_val) in ["None", "-", "0", "0.0"]:
            print("Warning: Missing stop loss. Applying safety default stop loss.")
            if current_price:
                if intent["side"] == "buy":
                    intent["stop_loss"] = current_price * 0.95
                elif intent["side"] == "sell":
                    intent["stop_loss"] = current_price * 1.05
            else:
                print("Cannot determine safe stop loss without current price. Aborting.")
                continue

        # 5. Execute Intent
        print(f"Executing {intent['side']} {intent['symbol']} via {provider}...")
        temp_intent_file = f"temp_execute_{intent['symbol']}.json"
        with open(temp_intent_file, "w") as f:
            json.dump(intent, f)

        result = run_command(["execute-intent", "--provider", provider, "--input", temp_intent_file])

        if os.path.exists(temp_intent_file):
            os.remove(temp_intent_file)

        if result:
            exec_res = result[0] if isinstance(result, list) else result
            status = exec_res.get("status", "unknown")

            print(f"Execution response: {status}")

            # 6. Monitor Slippage
            expected_price = intent.get("limit_price") or current_price
            slippage = None

            if expected_price and exec_res.get("provider_order_id"):
                _, slippage = monitor_execution(provider, exec_res["provider_order_id"], expected_price)

            # 7. Reporting
            log_trade(intent, exec_res, slippage)
        else:
            print("Execution failed.")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Execution Agent")
    parser.add_argument("--intent", required=True, help="Path to intent JSON file")
    args = parser.parse_args()

    execute_agent(args.intent)
