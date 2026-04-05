# 🗣️ Echo: Getting Started example is confusing

## 🤦 The Confusion:
Tried to run the `generate-signals` example in the README and the output was empty (`[]`). I thought I did something wrong, or the market was closed, or the command was broken! Also, I kept hitting weird jargon like "OHLCV", "RAG", "TWAP", and "VWAP".

## 🕵️ The Reality:
Turns out the `generate-signals` command only outputs something if a condition is met on the *exact latest candle*. If the market is moving sideways, it returns nothing. Also, the documentation uses technical finance acronyms without plain English explanations.

## 💡 The Fix:
Provide a `dummy_data.json` that guarantees a signal for the `BollingerBands` strategy so the README example actually shows a real output structure. Also, please explain jargon: OHLCV (price data), RAG (search history), TWAP (time spreading), VWAP (volume spreading).

---

# 🗣️ Echo: story_demo example is broken

## 🤦 The Confusion:
Tried to run the `story_demo` example mentioned in the README with `cargo run --features nova --example story_demo` but it failed with `error: no example target named 'story_demo' in default-run packages`. I thought I did something wrong or needed to install something else!

## 🕵️ The Reality:
Turns out the `story_demo` example doesn't actually exist in the `examples` folder of the repository. The README mentions it, but the code isn't there.

## 💡 The Fix:
Either add the `story_demo` example to the `examples/` directory so the command works, or remove the reference to it from the README to prevent confusing new users.
