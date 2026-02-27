import subprocess
import json
import os

CLI_PATH = "./target/release/thales-cli"

def run_command(args):
    cmd = [CLI_PATH] + args
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"Error: {result.stderr}")
        return None

    try:
        data = json.loads(result.stdout)
        if data.get("status") == "ok":
            return data.get("data")
        else:
            print(f"CLI Error: {data}")
            return None
    except Exception as e:
        print(f"Exception: {e}")
        return None

def test_fetch_price():
    # Test Paper (Simulation)
    print("Testing Paper Provider...")
    bars = run_command(["fetch-market-data", "--provider", "paper", "--symbol", "BTC/USD", "--timeframe", "1m"])
    if bars and len(bars) > 0:
        last_close = bars[-1]["close"]
        print(f"Paper BTC/USD Close: {last_close}")
    else:
        print("Failed to fetch Paper BTC/USD")

if __name__ == "__main__":
    test_fetch_price()
