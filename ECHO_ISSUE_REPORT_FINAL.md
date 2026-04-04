# 🗣️ Echo: Getting Started example is broken

## 🤦 The Confusion:
Tried to run the `generate-signals` example in the README and the output was empty (`[]`). I thought I did something wrong, or the market was closed, or the command was broken! Also, I kept hitting weird jargon like "OHLCV", "RAG", "TWAP", and "VWAP".

## 🕵️ The Reality:
Turns out the `generate-signals` command only outputs something if a condition is met on the *exact latest candle*. If the market is moving sideways, it returns nothing. Also, the documentation uses technical finance acronyms without plain English explanations.

## 💡 The Fix:
Provide a `dummy_data.json` that guarantees a signal for the `BollingerBands` strategy so the README example actually shows a real output structure. Also, please explain jargon: OHLCV (price data), RAG (search history), TWAP (time spreading), VWAP (volume spreading).
