# 🗣️ Echo: Getting Started example is confusing

## 🤦 The Confusion:
I was trying to run the quick start examples in `README.md`. I copy-pasted the `fetch-market-data` command and got some data. Then I ran the `generate-signals` command exactly as shown:
`cargo run -p thales-cli -- generate-signals --input market_data.json --strategy BollingerBands > signals.json`
I opened `signals.json` expecting to see trade signals, but the `data` array was totally empty: `{"status":"ok","errors":[],"warnings":["No signals triggered..."],"data":[]}`.
I thought I did something wrong, or the market was closed, or the command was broken!

Also, reading through the docs, I kept hitting weird jargon. What on earth is "TWAP" or "VWAP"? I'm just trying to make a trade, not get a PhD in finance!

## 🕵️ The Reality:
It turns out the `generate-signals` command only outputs something if a condition is met on the *exact latest candle*. If the market is just moving sideways right now, it returns nothing. The CLI does include a warning about this, but outputting an empty array on the very first "Quick Start" example makes it look like it failed silently.

On the bright side, when I messed up the commands on purpose (like using `--timeframe 99x` or `--provider invalid`), the error messages were actually readable ("provider error: invalid timeframe: 99x", "validation error: Unsupported provider: invalid") instead of just saying "Error: 2".

## 💡 The Fix:
1. Provide a `dummy_data.json` in the repo that is mathematically *guaranteed* to trigger a signal for the `BollingerBands` strategy so the README example actually shows a real output structure. Or change the default example to use the `backtest` command first, which always outputs a list of past trades.
2. Please remove or explain jargon like "TWAP"/"VWAP" (just say "time/volume spreading"). Keep it simple!
