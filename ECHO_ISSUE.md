# 🗣️ Echo: Getting Started example is broken

## 🤦 The Confusion:
Tried to run the `generate-signals` command shown in the "Quick Start" / README `generate-signals` section.
I fetched the market data as instructed, ran the signal generator, and I just got an empty `[]` array in `signals.json`.
I thought it was broken! I followed the README exactly, and it resulted in nothing. Why would you show an example that doesn't output anything?

## 🕵️ The Reality:
Turns out the `generate-signals` command *only* outputs a signal if one happens to trigger on the *exact latest candle* of the data fetched. Since market data is mostly sideways or not triggering a Bollinger Bands signal exactly right now, it returns `[]`.
The README has a "Note" explaining this under "Troubleshooting Empty Signals", but as a new user, I just copy-pasted the command and felt like it was broken when it produced an empty file.

## 💡 The Fix:
Add a huge banner/warning right before or in the code block itself, or change the default example to use the `backtest` command first which *guarantees* you see a list of trades, so the user sees the output structure immediately without having to be lucky with real-time market conditions. Or provide a `dummy_data.json` in the repo that is guaranteed to produce a signal for the example.

Another point: The JSON envelope overhead makes it annoying to read. We should really consider adding a `--raw` flag or similar if we want users to be able to pipe or read this without parsing `{"data": [...]}`.
