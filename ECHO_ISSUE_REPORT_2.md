# 🗣️ Echo: File not found error message is unhelpful

## 🤦 **The Confusion:**
When trying to run `execute-intent` with an input file that doesn't exist (e.g. `signals.json` as shown in the README, but I hadn't generated it yet), the tool simply spits out `{"status":"error","errors":["io error: No such file or directory (os error 2)"],"warnings":[],"data":null}`. I had to look at `os error 2` to figure out what was wrong.

## 🕵️ **The Reality:**
The CLI does not wrap the IO error with a helpful context. It just bubbles up the raw OS error.

## 💡 **The Fix:**
Provide a more user-friendly error message, e.g., "Could not open the input file 'signals.json'. Please ensure it exists."
