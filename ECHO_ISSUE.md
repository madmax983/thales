# 🗣️ Echo: Getting Started example is broken

## 🤦 The Confusion:
Tried to run the `generate-signals` quick start example in the README. The JSON output had `"status":"ok"` but `"data":[]`. I thought the command was broken or I did something wrong because no signals were generated.

Also, the documentation is full of financial jargon like OHLCV, RAG, TWAP, and VWAP that I don't understand.

## 🕵️ The Reality:
Turns out `generate-signals` only outputs a signal if a trading condition is met on the *exact latest candle*. If the market is moving sideways, it outputs nothing. The error messages for actual failures (like invalid timeframes or providers) were readable, though!

## 💡 The Fix:
Add a huge banner in README saying 'NO SIGNALS DOES NOT MEAN BROKEN'. Or better yet, change the quick start example to use `backtest` or provide a `dummy_data.json` that guarantees a signal. Also, replace jargon like OHLCV, RAG, TWAP, and VWAP with plain-English explanations (e.g., "price data", "search history", "time/volume spreading").
