import json
import subprocess
import os
import tempfile
import time
from datetime import datetime

CLI_PATH = "./target/debug/thales-cli"

def create_bars(symbol="BTCUSD", timeframe="1h", count=50, pattern="breakout"):
    bars = []
    now = int(time.time() * 1000)
    price = 100.0

    # Generate data that creates specific conditions
    for i in range(count):
        ts = now - (count - i) * 3600000

        if pattern == "breakout":
            # Flat then breakout at end to trigger Bollinger Band
            if i < count - 5:
                # Stable oscillating slightly
                price = 100.0 + (i % 2) * 0.5
            else:
                # Huge spike up -> Close > Upper Band -> Sell Signal (Mean Reversion)
                price += 5.0
        elif pattern == "chasing":
            # Continuous run up
            price += 2.0

        bars.append({
            "symbol": symbol,
            "market": "crypto",
            "timeframe": timeframe,
            "timestamp_unix_ms": ts,
            "open": price - 1.0,
            "high": price + 2.0,
            "low": price - 2.0,
            "close": price,
            "volume": 1000.0 + i
        })

    return {"schema_version": "v0", "bars": bars}

def run_cli(bars_data, history_data=None, strategy="BollingerBands"):
    with tempfile.NamedTemporaryFile(mode='w', delete=False) as bars_file:
        json.dump(bars_data, bars_file)
        bars_path = bars_file.name

    history_path = None
    if history_data:
        with tempfile.NamedTemporaryFile(mode='w', delete=False) as hist_file:
            json.dump(history_data, hist_file)
            history_path = hist_file.name

    cmd = [CLI_PATH, "generate-signals", "--input", bars_path, "--strategy", strategy]
    if history_path:
        cmd.extend(["--history", history_path])

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        output = json.loads(result.stdout)
        return output
    except subprocess.CalledProcessError as e:
        print(f"Error running CLI: {e.stderr}")
        return None
    except json.JSONDecodeError as e:
        print(f"Error parsing JSON: {e}")
        print(f"Output: {result.stdout}")
        return None
    finally:
        if os.path.exists(bars_path):
            os.remove(bars_path)
        if history_path and os.path.exists(history_path):
            os.remove(history_path)

def test_breakout_signal():
    print("\n--- Test 1: Breakout Signal Generation (Output Format & Risk Rules) ---")
    bars = create_bars(pattern="breakout")

    # Expect Sell Signal (Mean Reversion on spike)
    output = run_cli(bars, strategy="BollingerBands")

    if output and output.get("status") == "ok":
        signals = output.get("data", [])
        if len(signals) > 0:
            print("PASS: Signal generated.")
            sig = signals[0]
            print(f"  Signal: {sig['side'].upper()} {sig['symbol']} ({sig.get('signal_type', 'Unknown')})")

            # Check Verification Requirements
            if sig.get("stop_loss") is not None:
                print(f"PASS: Stop Loss present: {sig['stop_loss']}")
            else:
                print("FAIL: Missing Stop Loss")

            if sig.get("take_profit") is not None:
                print(f"PASS: Take Profit present: {sig['take_profit']}")
            else:
                print("FAIL: Missing Take Profit")

            # Size Hint
            try:
                size = float(sig.get("size_hint", "0"))
                if size > 0:
                    print(f"PASS: Size Hint valid (>0): {size}")
                else:
                    print(f"FAIL: Invalid Size Hint: {size}")
            except:
                if sig.get("size_hint") == "max":
                     print(f"PASS: Size Hint is 'max'")
                else:
                     print(f"FAIL: Size Hint parse error: {sig.get('size_hint')}")

            # Rationale
            if "BollingerBands" in sig["rationale"]:
                print("PASS: Rationale contains strategy info.")
            else:
                print(f"FAIL: Rationale missing strategy info: {sig['rationale']}")

        else:
            print("FAIL: No signal generated for breakout pattern.")
            print(output)
    else:
        print("FAIL: CLI execution failed.")

def test_daily_limit():
    print("\n--- Test 2: Daily Signal Limit (Max 3 per day) ---")
    bars = create_bars(pattern="breakout")

    # Create history with 3 trades TODAY
    now = int(time.time() * 1000)
    history = []
    # Mocking analysis structure required by RAG
    analysis_mock = {
        "symbol": "BTCUSD", "market": "crypto", "regime": "Trending Up", "volatility": "Low",
        "sentiment": "Neutral", "patterns": [], "key_levels": [], "confidence": 0.5, "timestamp_unix_ms": now
    }

    for i in range(3):
        history.append({
            "intent": {
                "symbol": "BTCUSD",
                "intent_id": f"test_{i}",
                "market": "crypto",
                "side": "buy",
                "size_hint": "1",
                "rationale": "test",
                "schema_version": "v0",
                "horizon": "1d",
                "invalidation": "none",
                "order_type": "market",
                "time_in_force": "day",
                "confidence": 0.5
            },
            "market_analysis": analysis_mock,
            "outcome": 1.0
        })

    output = run_cli(bars, history_data=history)
    signals = output.get("data", [])

    if len(signals) == 0:
        print("PASS: Signal suppressed due to daily limit (3 existing trades today).")
    else:
        print(f"FAIL: Signal generated despite limit. Count: {len(signals)}")

def test_rag_context():
    print("\n--- Test 3: RAG Context & Confidence Boost ---")
    bars = create_bars(pattern="breakout")

    # Create history with HIGH WIN RATE (yesterday)
    now = int(time.time() * 1000)
    history = []

    # We need the analysis of the CURRENT bars to match the HISTORY bars for RAG to pick it up.
    # The synthetic bars create "Trending Up" (due to spike) and likely "Medium" or "High" volatility.
    # Let's create history entries that match "Trending Up" to ensure they are found.

    analysis_mock = {
        "symbol": "BTCUSD", "market": "crypto", "regime": "Trending Up", "volatility": "Low", # Assuming Low/Medium match
        "sentiment": "Neutral", "patterns": [], "key_levels": [], "confidence": 0.5,
        "timestamp_unix_ms": now - 86400000 # Yesterday
    }

    # Note: RAG finds trades with Same Market, Regime, Volatility.
    # Our synthetic data might produce "High" volatility due to spike.
    # Let's try to match what the CLI produces.
    # Ideally we'd run analyze-market first, but let's just create diverse history to cover bases.

    for vol in ["Low", "Medium", "High"]:
        for i in range(2):
            hist_item = analysis_mock.copy()
            hist_item["volatility"] = vol
            history.append({
                "intent": {
                    "symbol": "BTCUSD",
                    "intent_id": f"test_{vol}_{i}",
                    "market": "crypto",
                    "side": "buy",
                    "size_hint": "1",
                    "rationale": "test",
                    "schema_version": "v0",
                    "horizon": "1d",
                    "invalidation": "none",
                    "order_type": "market",
                "time_in_force": "day",
                "confidence": 0.5
                },
                "market_analysis": hist_item,
                "outcome": 100.0 # WIN
            })

    output = run_cli(bars, history_data=history)
    signals = output.get("data", [])

    if len(signals) > 0:
        sig = signals[0]
        # print(f"Rationale: {sig['rationale']}")

        if "similar past trades" in sig["rationale"]:
             print(f"PASS: Rationale includes RAG summary: '{sig['rationale']}'")
        else:
             print(f"FAIL: Rationale missing RAG summary: {sig['rationale']}")

        if "Boosted" in sig["rationale"] or "Penalized" in sig["rationale"]:
            print("PASS: Confidence adjusted based on history.")
        else:
            # It might not boost if win rate isn't perfect for the *specific* matched subset
            # But with 100% wins in history, it should boost if it finds any.
            print(f"WARN: Confidence might not have been adjusted (Check matches).")
    else:
        print("FAIL: No signal generated.")

if __name__ == "__main__":
    if not os.path.exists(CLI_PATH):
        print("Please build thales-cli first: cargo build -p thales-cli")
        exit(1)

    test_breakout_signal()
    test_daily_limit()
    test_rag_context()
