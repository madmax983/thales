import re
import os
import datetime
import json

SIGNALS_PATH = "Signals.md"
REGIME_LOG = "Market_Regime.md"
VOLATILITY_LOG = "Volatility_Regime.md"
RESEARCH_LOG = "Market_Research.md"

def parse_signals():
    if not os.path.exists(SIGNALS_PATH):
        return []

    with open(SIGNALS_PATH, "r") as f:
        content = f.read()

    entries = []
    # Split by ## Market Analysis Report or similar headers
    chunks = re.split(r"\n## ", content)

    for chunk in chunks:
        if not chunk.strip(): continue

        # We look for the JSON block which is most reliable
        json_match = re.search(r"```json\s*(\{.*?\})\s*```", chunk, re.DOTALL)
        if not json_match:
            continue

        try:
            raw_data = json.loads(json_match.group(1))
            # Handle envelope if present
            if "data" in raw_data and isinstance(raw_data["data"], dict):
                entries.append(raw_data["data"])
            else:
                entries.append(raw_data)
        except:
            continue

    return entries

def format_date(ts_ms):
    return datetime.datetime.fromtimestamp(ts_ms / 1000).strftime("%Y-%m-%d %H:%M:%S")

def append_if_new(filepath, line, unique_key):
    """
    Appends line to file if unique_key is not found in file.
    unique_key could be the formatted date + symbol.
    """
    if not os.path.exists(filepath):
        with open(filepath, "w") as f:
            if "Regime" in filepath:
                f.write("# Regime Log\n")
            elif "Research" in filepath:
                f.write("# Market Research Log\n")

    with open(filepath, "r") as f:
        content = f.read()

    if unique_key in content:
        return # Already exists

    with open(filepath, "a") as f:
        f.write(line + "\n")

def update_regime_log(entries):
    for entry in entries:
        ts = entry.get("timestamp_unix_ms")
        if not ts: continue
        date_str = format_date(ts)
        symbol = entry.get("symbol")
        regime = entry.get("regime")
        sentiment = entry.get("sentiment")
        conf = entry.get("confidence", 0.0)

        line = f"| {date_str} | {symbol} | {regime} | {sentiment} | {conf:.2f} |"
        unique_key = f"| {date_str} | {symbol} |"

        append_if_new(REGIME_LOG, line, unique_key)

def update_volatility_log(entries):
    for entry in entries:
        ts = entry.get("timestamp_unix_ms")
        if not ts: continue
        date_str = format_date(ts)
        symbol = entry.get("symbol")
        vol = entry.get("volatility")
        conf = entry.get("confidence", 0.0)

        line = f"| {date_str} | {symbol} | {vol} | {conf:.2f} |"
        unique_key = f"| {date_str} | {symbol} |"

        append_if_new(VOLATILITY_LOG, line, unique_key)

def update_research_log(entries):
    for entry in entries:
        ts = entry.get("timestamp_unix_ms")
        if not ts: continue
        date_str = format_date(ts)
        symbol = entry.get("symbol")
        research = entry.get("research_summary", "No research.")
        news = entry.get("news_summary", "No news.")

        header = f"## {date_str} - {symbol}"
        block = f"{header}\n**Research**:\n{research}\n\n**News**:\n{news}\n"

        append_if_new(RESEARCH_LOG, block, header)

def main():
    entries = parse_signals()
    # Sort by timestamp ascending to append in order
    entries.sort(key=lambda x: x.get("timestamp_unix_ms", 0))

    update_regime_log(entries)
    update_volatility_log(entries)
    update_research_log(entries)
    print("Logs updated.")

if __name__ == "__main__":
    main()
