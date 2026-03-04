import json
import os
from datetime import datetime

PORTFOLIO_FILE = "portfolio.md"

def append_to_portfolio_skipped(symbol, reason):
    now_str = datetime.utcnow().strftime("%Y-%m-%d %H:%M:%S")
    line = f"| {now_str} | {symbol} | NO_REF | {reason} |\n"
    with open(PORTFOLIO_FILE, "a") as f:
        f.write(line)

assets = ['btc', 'eth', 'spy']
for asset in assets:
    files = [f for f in os.listdir('.') if f.startswith(f"{asset}_") and f.endswith('.json') and 'data' not in f and 'analysis' not in f and 'execution' not in f and 'signal' not in f]

    sides = set()
    for file in files:
        try:
            with open(file, 'r') as f:
                data = json.load(f)
                if 'data' in data and data['data'] is not None and len(data['data']) > 0:
                    side = data['data'][0].get('side')
                    if side:
                        sides.add(side)
        except Exception as e:
            print(f"Error parsing {file}: {e}")

    symbol = f"{asset.upper()}USD" if asset != 'spy' else "SPY"
    if len(sides) > 1:
        print(f"Conflict detected for {symbol}. Sides: {sides}. Halting trading.")
        append_to_portfolio_skipped(symbol, f"Conflict: Multiple strategies gave conflicting signals ({sides})")
    elif len(sides) == 1:
        print(f"No conflict for {symbol}. Valid side: {list(sides)[0]}.")
    else:
        print(f"No valid signals for {symbol}.")
