# 🗣️ Echo: Developer Experience Audit

## 🤦 The Confusion:
I was trying to run the quick start examples in `README.md`. I copy-pasted the `fetch-market-data` command and got some data. Then I ran the `generate-signals` command exactly as shown:
`cargo run -p thales-cli -- generate-signals --input dummy_data.json --strategy BollingerBands > signals.json`
Also, reading through the docs, I kept hitting weird jargon. What on earth is "TWAP" or "VWAP"? I'm just trying to make a trade, not get a PhD in finance!
And if I accidentally type a bad file name like `invalid_file.json`, I get a messy error: `io error: Could not open input file 'invalid_file.json': No such file or directory (os error 2)`. 'os error 2' isn't helpful to me.

## 🕵️ The Reality:
It turns out the `generate-signals` command only outputs something if a condition is met on the exact latest candle. If the market is just moving sideways right now, it returns nothing.
On the bright side, when I use a dummy file that's mathematically set up for it, it does actually spit out a signal.

## 💡 The Fix:
1. Please remove or explain jargon like "TWAP"/"VWAP" (just say "time/volume spreading"). Keep it simple!
2. Please wrap the file read error so it doesn't just spew '(os error 2)' at the user.
