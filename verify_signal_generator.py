import json
import subprocess
import os
import math

CLI_PATH = "./target/release/thales-cli"
BARS_FILE = "verify_bars.json"
HISTORY_FILE = "verify_history.json"

def create_synthetic_data():
    """Creates synthetic OHLCV data to trigger a Bollinger Bands Buy signal."""
    bars = []
    now = 100000

    # 20 bars of stable price (Mean ~100)
    for i in range(20):
        bars.append({
            "symbol": "TEST",
            "market": "equities",
            "timeframe": "1m",
            "timestamp_unix_ms": now + i * 60000,
            "open": 100.0,
            "high": 101.0,
            "low": 99.0,
            "close": 100.0,
            "volume": 1000.0
        })

    # 21st bar: Drop to 90 (Below Lower Band)
    bars.append({
        "symbol": "TEST",
        "market": "equities",
        "timeframe": "1m",
        "timestamp_unix_ms": now + 20 * 60000,
        "open": 100.0,
        "high": 100.0,
        "low": 90.0,
        "close": 90.0,
        "volume": 5000.0
    })

    envelope = {
        "status": "ok",
        "errors": [],
        "warnings": [],
        "data": {
            "schema_version": "v0",
            "bars": bars
        }
    }

    with open(BARS_FILE, "w") as f:
        json.dump(envelope, f)

def create_dummy_history():
    """Creates a dummy history file to test RAG."""
    # Entry for RAG lookup
    entry = {
        "intent": {
            "symbol": "TEST",
            "market": "equities",
            "side": "buy",
            "confidence": 0.8,
            "strategy": "BollingerBandsMeanReversion",
            "intent_id": "test_hist",
            "size_hint": "100",
            "horizon": "1d",
            "rationale": "Test history",
            "invalidation": "none",
            "schema_version": "v0",
            "order_type": "market",
            "time_in_force": "day"
        },
        "market_analysis": {
            "symbol": "TEST",
            "market": "equities",
            "regime": "Trending Up", # Should match analysis if we align it, but simplified here
            "sentiment": "Neutral",
            "patterns": [],
            "key_levels": [],
            "volatility": "Low",
            "confidence": 0.5,
            "research_summary": None,
            "news_summary": None,
            "recommendation": None,
            "atr": None,
            "timestamp_unix_ms": 0
        },
        "outcome": 1.0 # Win
    }

    with open(HISTORY_FILE, "w") as f:
        json.dump([entry], f)

def run_signal_generation():
    """Runs the CLI command and verifies output."""
    cmd = [
        CLI_PATH,
        "generate-signals",
        "--input", BARS_FILE,
        "--strategy", "BollingerBands",
        "--history", HISTORY_FILE,
        "--risk", "100.0"
    ]

    print(f"Running: {' '.join(cmd)}")
    result = subprocess.run(cmd, capture_output=True, text=True)

    if result.returncode != 0:
        print("Command failed!")
        print("Stdout:", result.stdout)
        print("Stderr:", result.stderr)
        return False

    try:
        output = json.loads(result.stdout)
        if output["status"] != "ok":
            print("CLI returned error status:", output)
            return False

        intents = output["data"]
        if not intents:
            print("No signals generated.")
            return False

        intent = intents[0]
        print("\nGenerated Signal:")
        print(json.dumps(intent, indent=2))

        # Verifications
        assert intent["symbol"] == "TEST"
        assert intent["side"] == "buy"
        assert intent["stop_loss"] is not None
        assert intent["take_profit"] is not None

        size = float(intent["size_hint"])
        assert size > 0.0
        assert math.isfinite(size)

        # Rationale check (History or Analysis)
        print("Rationale:", intent["rationale"])

        print("\nVerification Successful!")
        return True

    except Exception as e:
        print(f"Verification failed: {e}")
        return False

def main():
    # Build CLI first? Assuming it is built or we run 'cargo build' before.
    # The instruction says "CLI_PATH = ./target/release/thales-cli"
    # We should ensure it exists.
    if not os.path.exists(CLI_PATH):
        print("Building CLI...")
        subprocess.run(["cargo", "build", "--release", "-p", "thales-cli"], check=True)

    create_synthetic_data()
    create_dummy_history()

    if run_signal_generation():
        print("Signal Generator is working correctly.")
    else:
        print("Signal Generator verification failed.")
        exit(1)

    # Cleanup
    if os.path.exists(BARS_FILE):
        os.remove(BARS_FILE)
    if os.path.exists(HISTORY_FILE):
        os.remove(HISTORY_FILE)

if __name__ == "__main__":
    main()
