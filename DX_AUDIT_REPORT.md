# 🗣️ Echo: DX Audit Report

## 📢 Complaint: "Signals? What Signals?"

**Title:** 🗣️ Echo: Getting Started example is confusing (No Signals Generated)

**Description:**

*   🤦 **The Confusion:**
    *   I followed the `README.md` step-by-step.
    *   I ran the `fetch-market-data` command. It worked! I felt powerful.
    *   I ran the `generate-signals` command as shown:
        ```bash
        cargo run -p thales-cli -- generate-signals \
          --input market_data.json \
          --strategy BollingerBandsMeanReversion > signals.json
        ```
    *   I opened `signals.json` expecting to see my future riches.
    *   **Result:** `{"status":"ok","data":[]}`. Empty. Nada.
    *   I thought I broke it. I thought the market was closed. I thought the strategy was bad (it is, but that's not the point).

*   🕵️ **The Reality:**
    *   The `generate-signals` command *only* outputs a signal if one triggers on the *exact latest candle*.
    *   The `stderr` (which I ignored because I was redirecting stdout) actually had a useful log: `Skipping sell signal for XXBTZUSD due to low confidence`. But I missed it because the success output (`status: ok`) implies everything is fine.
    *   The README has a small "Note" about this, but I am Echo. I don't read Notes. I read code blocks.

*   💡 **The Fix:**
    *   **Better Default Example:** Use a strategy or mock data in the README example that *guarantees* a signal, so the user sees `TradeIntent` structure immediately.
    *   **Verbose Output:** If `data` is empty, the `warnings` field in the JSON envelope should explain *why*. e.g., `"warnings": ["No signals triggered for latest candle (2025-02-18 12:00)"]`.
    *   **CLI UX:** If running interactively (detected via tty), print "No signals generated" to stderr prominently.

## 🚧 Friction Points

1.  **"Silent Failure" on Signal Generation:**
    *   As mentioned above, an empty array `[]` looks like "it worked but nothing happened," which is indistinguishable from "it didn't work and I don't know why."

2.  **JSON Envelope Overhead:**
    *   I like piping data. `cat market_data.json | thales-cli backtest ...` is cool.
    *   But manually reading `{"data": ...}` is annoying when I just want the list of trades.
    *   *Suggestion:* Consider a `--raw` or `--json-lines` flag for piping pure data between tools without unwrapping the envelope.

3.  **Help Text vs. Reality:**
    *   `execute-intent` takes a provider argument.
    *   If I pass a bogus provider (`invalid`), I get `Unsupported provider`. Good.
    *   If I pass a bogus file (`missing.json`), I get `No such file`. Good.
    *   *Nitpick:* The CLI help for `generate-signals` defaults to `BollingerBands`, but the README example uses `BollingerBandsMeanReversion`. Consistency would reduce copy-paste anxiety.

## ✅ The Good Stuff

*   **It Compiles:** The examples in the README *actually run*. This puts you in the top 10% of Rust projects.
*   **Error Messages:** "Unsupported provider" and "Invalid timeframe" are readable. They don't just say `Error: 1`.
*   **Performance:** It's fast. I didn't have time to get bored waiting for the backtest.

---
*Signed, Echo 🗣️*
# 🗣️ Echo: Getting Started example is confusing

## 🤦 The Confusion:
I was trying to run the quick start examples in `README.md`. I copy-pasted the `fetch-market-data` command and got some data. Then I ran the `generate-signals` command exactly as shown:
`cargo run -p thales-cli -- generate-signals --input market_data.json --strategy BollingerBands > signals.json`
I opened `signals.json` expecting to see trade signals, but the `data` array was totally empty: `{"status":"ok","errors":[],"warnings":["No signals triggered..."],"data":[]}`.
I thought I did something wrong, or the market was closed, or the command was broken!

Also, reading through the docs, I kept hitting weird jargon. What on earth is "OHLCV", "RAG", "TWAP", or "VWAP"? I'm just trying to make a trade, not get a PhD in finance!

## 🕵️ The Reality:
It turns out the `generate-signals` command only outputs something if a condition is met on the *exact latest candle*. If the market is just moving sideways right now, it returns nothing. The CLI does include a warning about this, but outputting an empty array on the very first "Quick Start" example makes it look like it failed silently.

On the bright side, when I messed up the commands on purpose (like using `--timeframe 99x` or `--provider invalid`), the error messages were actually readable ("provider error: invalid timeframe: 99x", "Unsupported provider: invalid") instead of just saying "Error: 2".

## 💡 The Fix:
1. Provide a `dummy_data.json` in the repo that is mathematically *guaranteed* to trigger a signal for the `BollingerBands` strategy so the README example actually shows a real output structure. Or change the default example to use the `backtest` command first, which always outputs a list of past trades.
2. Please remove or explain jargon like "OHLCV" (just say "price data"), "RAG" (just say "search history"), and "TWAP"/"VWAP" (just say "time/volume spreading"). Keep it simple!